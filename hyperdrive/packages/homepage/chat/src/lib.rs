// HYPERWARE CHAT APPLICATION
// A mobile-first chat application for the Hyperware platform
// Supporting 1:1 DMs, Group chats (TODO), and Voice calls (TODO)

use base64::{engine::general_purpose, Engine as _};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::{channel::mpsc::UnboundedReceiver, pin_mut, select, FutureExt, StreamExt};
use hyperapp_macro::*;
use hyperware_crdt::yrs::{Decode, Encode, StateVector};
use hyperware_process_lib::{
    homepage::add_to_homepage,
    http::server::WsMessageType,
    hyperapp::{
        self, get_path, send, set_response_status, sleep, source, spawn, AppSendError, SaveOptions,
    },
    our, vfs, Address, Capability, LazyLoadBlob, ProcessId, Request, Request as ProcessRequest,
};
use std::cmp::Ordering;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// Import generated RPC functions from caller-utils
use chat_caller_utils::chat::{
    receive_chat_creation_remote_rpc, receive_message_ack_remote_rpc,
    receive_message_deletion_remote_rpc, receive_message_edit_remote_rpc,
    receive_message_remote_rpc, receive_profile_update_remote_rpc, receive_reaction_remote_rpc,
    receive_reaction_remove_remote_rpc,
};
use chat_caller_utils::ChatMessage as CUChatMessage;
use chat_caller_utils::UserProfile as CUUserProfile;
use homepage_caller_utils as chat_caller_utils;

mod crdt;
mod groups;
pub mod logging;
mod pubsub;
mod replication;
mod search;
mod types;
mod ws;

pub use crdt::GroupDocState;
pub use types::*;

#[cfg(feature = "test-helpers")]
pub mod test_exports {
    pub use crate::crdt::{DeliveryCursor, Group, GroupRoutingConfig, SubscriberSyncState};
    pub use crate::types::{BrokerEnvelope, ChatState, ReplicationKind, ReplicationTask};
}

use crate::crdt::{
    GroupId, GroupPermissions, GroupVisibility, MembershipDecisionStatus, MembershipStatus, NodeId,
};

const OUR_PROCESS_ID: (&str, &str, &str) = ("chat", "homepage", "sys");
const CONTACTS_PROCESS_ID: (&str, &str, &str) = ("contacts", "contacts", "sys");
const CONTACTS_FIELD_NICKNAME: &str = "nickname";
const CONTACTS_FIELD_BASE_ADDRESS: &str = "base_address";
// Replication RPC timeout to keep admin/test calls responsive.
const REPL_RPC_TIMEOUT_SECS: u64 = 2;
const ICON: &str = include_str!("./icon");

// Helper function to enforce one-way status transitions
fn safe_update_message_status(current: &MessageStatus, new: MessageStatus) -> MessageStatus {
    use MessageStatus::*;

    // Define valid transitions
    match (current, &new) {
        // From Sending, can go to Sent, Delivered, or Failed
        (Sending, Sent) | (Sending, Delivered) | (Sending, Failed) => new,

        // From Sent, can only go to Delivered or Failed
        (Sent, Delivered) | (Sent, Failed) => new,

        // From Delivered, cannot change (terminal state)
        (Delivered, _) => {
            log_debug!(
                "WARNING: Attempted invalid status transition from Delivered to {:?}",
                new
            );
            current.clone()
        }

        // From Failed, cannot change (terminal state)
        (Failed, _) => {
            log_debug!(
                "WARNING: Attempted invalid status transition from Failed to {:?}",
                new
            );
            current.clone()
        }

        // Any backwards transition is invalid
        _ => {
            log_debug!(
                "WARNING: Attempted invalid status transition from {:?} to {:?}",
                current,
                new
            );
            current.clone()
        }
    }
}

// Helper functions for compression
fn compress_data(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| format!("Compression error: {}", e))?;
    encoder
        .finish()
        .map_err(|e| format!("Compression finish error: {}", e))
}

fn decompress_data(compressed: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Decompression error: {}", e))?;
    Ok(decompressed)
}

// Helper functions for base64 encoding/decoding
fn base64_encode(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    general_purpose::STANDARD.decode(input)
}

/// Send the WIT `ReplicationWork` unit variant to ourselves. This wakes the
/// replication handler, which runs with a ~12s budget to drain queues.
async fn trigger_replication(target: Address) {
    let _ = chat_caller_utils::chat::replication_work_local_rpc(&target).await;
}

/// Push a snapshot immediately to a new member (bypassing the debounced queue).
/// This is called when a member is invited and approved, so they can bootstrap
/// without waiting for the replication scheduler's 250ms debounce.
fn spawn_immediate_snapshot_push(group_id: GroupId, peer: String) {
    spawn(async move {
        let self_addr = Address::from((our().node.as_str(), OUR_PROCESS_ID));
        let body = serde_json::to_vec(&serde_json::json!({
            "PushSnapshotToPeer": {
                "group_id": group_id,
                "peer": peer,
            }
        }))
        .unwrap_or_default();
        let req = Request::new()
            .target(self_addr)
            .body(body)
            .expects_response(5);
        let _ = send::<serde_json::Value>(req).await;
    });
}

/// Spawn the event + timer replication scheduler. It listens for wake signals
/// (new work enqueued) and also ticks every 5s as a safety net, sending the WIT
/// `ReplicationWork` variant to our own address. The receiver side (`replication_work`)
/// enforces the 12s budget for draining tasks.
fn start_replication_scheduler(wake_rx: ReplicationWakeRx) {
    let mut wake_rx = wake_rx.into_stream();
    let self_addr = Address::from((our().node.as_str(), OUR_PROCESS_ID));
    spawn(async move {
        let debounce = Duration::from_millis(250);
        let mut last_wake: Option<Instant> = None;
        loop {
            let wake = wake_rx.next().fuse();
            let tick = sleep(5000).fuse();
            pin_mut!(wake, tick);
            select! {
                _ = wake => {
                    let now = Instant::now();
                    let should_fire = last_wake
                        .map(|ts| now.duration_since(ts) >= debounce)
                        .unwrap_or(true);
                    if should_fire {
                        last_wake = Some(now);
                        trigger_replication(self_addr.clone()).await;
                    }
                }
                _ = tick => {
                    last_wake = None;
                    trigger_replication(self_addr.clone()).await;
                }
            }
        }
    });
}

pub(crate) fn log_crdt_event(
    doc_id: &str,
    context: &str,
    state_vector: &StateVector,
    update_len: Option<usize>,
) {
    let sv_len = state_vector.len();
    match update_len {
        Some(len) => log_debug!(
            "[CRDT][{}] context={} state_vector_len={} update_bytes={}",
            doc_id,
            context,
            sv_len,
            len
        ),
        None => log_debug!(
            "[CRDT][{}] context={} state_vector_len={}",
            doc_id,
            context,
            sv_len
        ),
    }
}

fn log_group_state_summary(doc_id: &str, context: &str, state: &GroupDocState) {
    let member_count = state.group.members.len();
    let hubs_count = state.group.hubs.active.len();
    let subs_count = state.group.subscribers.entries.len();
    let roles_count = state.group.roles.len();
    let sample_members: Vec<String> = state.group.members.keys().take(3).cloned().collect();
    log_debug!(
        "[CRDT][{}] context={} state_summary members={} hubs={} subs={} roles={} sample_members={:?}",
        doc_id, context, member_count, hubs_count, subs_count, roles_count, sample_members
    );
}

// Helper function to send push notification for a message
async fn send_push_notification_for_message(sender: &str, content: &str, chat_id: &str) {
    if cfg!(feature = "disable-notifications") {
        log_debug!("[NOTIFY] skipping push notification (disable-notifications feature enabled)");
        return;
    }
    let notify_started = Instant::now();
    // Send notification to notifications server (it will send to all registered devices)
    let notifications_address = Address::new(
        &our().node,
        ProcessId::new(Some("notifications"), "distro", "sys"),
    );

    // Truncate message for notification
    let truncated_content = if content.len() > 100 {
        format!("{}...", &content[..97])
    } else {
        content.to_string()
    };

    let notification_action = NotificationsAction::SendNotification {
        title: format!("Message from {}", sender),
        body: truncated_content,
        icon: Some("/icon-180.png".to_string()),
        data: Some(serde_json::json!({
            "url": format!("/chat#{}", chat_id),
            "chat_id": chat_id,
            "sender": sender,
            "appId": "chat:homepage:sys",
            "appLabel": "Chat"
        })),
    };

    // Send the notification request
    log_debug!("Sending notification to notifications:distro:sys");
    let request = Request::to(notifications_address.clone())
        .body(serde_json::to_vec(&notification_action).unwrap())
        .expects_response(5);
    if let Ok(body_str) = serde_json::to_string(&notification_action) {
        log_debug!(
            "[NOTIFY] sending to {} body_len={} body={}",
            notifications_address,
            body_str.len(),
            body_str
        );
    }

    match send::<NotificationsResponse>(request).await {
        Ok(resp) => {
            log_debug!("Push notification response: {:?}", resp);
            match resp {
                NotificationsResponse::NotificationSent => {
                    log_debug!("Push notification sent successfully");
                }
                NotificationsResponse::Err(e) => {
                    log_debug!("Notification server error: {}", e);
                    // Check if the error contains "EndpointNotValid"
                    if e.contains("EndpointNotValid") {
                        // Extract the endpoint URL from the error message
                        // Error format: "Failed to send to https://fcm.googleapis.com/fcm/send/...: EndpointNotValid"
                        if let Some(start) = e.find("https://") {
                            if let Some(end) = e[start..].find(':') {
                                let endpoint = &e[start..start + end];
                                log_debug!("Removing invalid endpoint: {}", endpoint);

                                // Send request to remove the invalid subscription
                                let remove_action = NotificationsAction::RemoveSubscription {
                                    endpoint: endpoint.to_string(),
                                };

                                let remove_request = Request::to(notifications_address.clone())
                                    .body(serde_json::to_vec(&remove_action).unwrap())
                                    .expects_response(5);
                                log_debug!(
                                    "[NOTIFY] removing invalid endpoint {} via {}",
                                    endpoint,
                                    notifications_address
                                );

                                // Fire and forget the removal request
                                spawn(async move {
                                    match send::<NotificationsResponse>(remove_request).await {
                                        Ok(NotificationsResponse::SubscriptionRemoved) => {
                                            log_debug!("Successfully removed invalid endpoint");
                                        }
                                        Ok(resp) => {
                                            log_debug!(
                                                "Unexpected response removing endpoint: {:?}",
                                                resp
                                            );
                                        }
                                        Err(e) => {
                                            log_debug!("Error removing invalid endpoint: {:?}", e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                _ => {
                    log_debug!("Unexpected notification response");
                }
            }
            log_debug!(
                "[NOTIFY_DIAG] send_push_notification ok elapsed_ms={}",
                notify_started.elapsed().as_millis()
            );
        }
        Err(e) => {
            log_debug!("Error sending notification request: {:?}", e);
            log_debug!(
                "[NOTIFY_DIAG] send_push_notification err elapsed_ms={}",
                notify_started.elapsed().as_millis()
            );
        }
    }
}

// Helper function to send push notification for a group message
async fn send_push_notification_for_group_message(
    sender: &str,
    content: &str,
    group_id: &str,
    group_name: &str,
) {
    if cfg!(feature = "disable-notifications") {
        log_debug!(
            "[NOTIFY] skipping group push notification (disable-notifications feature enabled)"
        );
        return;
    }
    let notify_started = Instant::now();
    let notifications_address = Address::new(
        &our().node,
        ProcessId::new(Some("notifications"), "distro", "sys"),
    );

    // Truncate message for notification
    let truncated_content = if content.len() > 100 {
        format!("{}...", &content[..97])
    } else {
        content.to_string()
    };

    let notification_action = NotificationsAction::SendNotification {
        title: format!("{} in {}", sender, group_name),
        body: truncated_content,
        icon: Some("/icon-180.png".to_string()),
        data: Some(serde_json::json!({
            "url": format!("/chat#group:{}", group_id),
            "group_id": group_id,
            "sender": sender,
            "appId": "chat:homepage:sys",
            "appLabel": "Chat"
        })),
    };

    log_debug!("[NOTIFY] Sending group notification to notifications:distro:sys");
    let request = Request::to(notifications_address.clone())
        .body(serde_json::to_vec(&notification_action).unwrap())
        .expects_response(5);

    match send::<NotificationsResponse>(request).await {
        Ok(resp) => {
            log_debug!("Group push notification response: {:?}", resp);
            match resp {
                NotificationsResponse::NotificationSent => {
                    log_debug!("Group push notification sent successfully");
                }
                NotificationsResponse::Err(e) => {
                    log_debug!("Group notification server error: {}", e);
                    // Handle invalid endpoint same as DM notifications
                    if e.contains("EndpointNotValid") {
                        if let Some(start) = e.find("https://") {
                            if let Some(end) = e[start..].find(':') {
                                let endpoint = &e[start..start + end];
                                log_debug!("Removing invalid endpoint: {}", endpoint);
                                let remove_action = NotificationsAction::RemoveSubscription {
                                    endpoint: endpoint.to_string(),
                                };
                                let remove_request = Request::to(notifications_address.clone())
                                    .body(serde_json::to_vec(&remove_action).unwrap())
                                    .expects_response(5);
                                spawn(async move {
                                    let _ = send::<NotificationsResponse>(remove_request).await;
                                });
                            }
                        }
                    }
                }
                _ => {
                    log_debug!("Unexpected group notification response");
                }
            }
            log_debug!(
                "[NOTIFY_DIAG] send_push_notification_for_group ok elapsed_ms={}",
                notify_started.elapsed().as_millis()
            );
        }
        Err(e) => {
            log_debug!("Error sending group notification request: {:?}", e);
            log_debug!(
                "[NOTIFY_DIAG] send_push_notification_for_group err elapsed_ms={}",
                notify_started.elapsed().as_millis()
            );
        }
    }
}

// HYPERAPP IMPLEMENTATION

#[hyperapp(
    name = "Chat",
    ui = Some(HttpBindingConfig::default()),
    endpoints = vec![
        Binding::Http {
            path: "/api",
            config: HttpBindingConfig::default(),
        },
        Binding::Ws {
            path: "/ws",
            config: WsBindingConfig::default(),
        },
        Binding::Http {
            path: "/public",
            config: HttpBindingConfig::new(false, false, false, None)
        },
        Binding::Http {
            path: "/files/*",
            config: HttpBindingConfig::default(),
        }
    ],
    save_config = SaveOptions::OnDiff,
    wit_world = "chat-ware-dot-hypr-v0"
)]
impl ChatState {
    #[init]
    async fn initialize(&mut self) {
        // Initialize with default profile
        if self.profile.name == "User" {
            let our_node = our().node.clone();
            self.profile.name = our_node.split('.').next().unwrap_or("User").to_string();
        }

        // Create VFS drive for storing chat files
        let package_id = our().package_id();
        match vfs::create_drive(package_id, "files", Some(5)) {
            Ok(drive_path) => {
                log_debug!("Created files drive at: {}", drive_path);
            }
            Err(e) => {
                log_debug!("Failed to create files drive (may already exist): {:?}", e);
            }
        }

        // Add a welcome chat if no chats exist
        if self.chats.is_empty() {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let welcome_chat = Chat {
                id: "system:welcome".to_string(),
                counterparty: "System".to_string(),
                messages: vec![ChatMessage {
                    id: format!("welcome_{}", timestamp),
                    sender: "System".to_string(),
                    content: "Welcome to Hyperware Chat! You can create new chats by clicking the + button.".to_string(),
                    timestamp,
                    sequence: Some(0),
                    status: MessageStatus::Delivered,
                    reply_to: None,
                    reactions: Vec::new(),
                    message_type: MessageType::Text,
                    file_info: None,
                    payment_info: None,
                }],
                last_activity: timestamp,
                unread_count: 0,
                is_blocked: false,
                notify: false,
                counterparty_profile: None,
            };

            self.chats
                .insert("system:welcome".to_string(), welcome_chat);
        }

        // Reconcile any legacy duplicate DM chats into canonical node-based IDs.
        let mut reconcile_nodes: HashSet<String> = HashSet::new();
        for chat in self.chats.values() {
            if !chat.id.starts_with("system:")
                && !chat.id.starts_with("browser:")
                && !chat.counterparty.is_empty()
                && chat.counterparty != our().node
            {
                reconcile_nodes.insert(chat.counterparty.clone());
            }
            for message in &chat.messages {
                if message.sender != our().node && message.sender != "System" {
                    reconcile_nodes.insert(message.sender.clone());
                }
            }
        }
        for node in reconcile_nodes {
            self.reconcile_dm_chats_for_counterparty(&node);
        }

        let existing_chat_ids: Vec<String> = self.chats.keys().cloned().collect();
        for chat_id in existing_chat_ids {
            self.ensure_sequence_state(&chat_id);
        }

        if let Some(delivery_rx) = self.delivery_rx.take() {
            let delivery_tx = self.delivery_tx.clone();
            let pending_deliveries = self.pending_deliveries.clone();
            spawn(async move {
                ChatState::run_delivery_worker(delivery_rx, delivery_tx, pending_deliveries).await;
            });
        }

        self.bootstrap_pending_deliveries();

        // Kick off replication worker loop (event-driven with periodic safety net)
        if let Some(wake_rx) = self.replication_wake_rx.take() {
            start_replication_scheduler(wake_rx);
        }

        self.rebuild_search_index();

        log_debug!(
            "Chat app initialized on node: {} with {} chats",
            our().node,
            self.chats.len()
        );
    }

    // CHAT MANAGEMENT ENDPOINTS

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn create_chat(&mut self, req: CreateChatReq) -> Result<Chat, String> {
        // Normalize chat ID to always be alphabetically sorted
        let chat_id = Self::normalize_chat_id(&our().node, &req.counterparty);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.reconcile_dm_chats_for_counterparty(&req.counterparty);

        // Get counterparty profile if we have it
        let counterparty_profile = self.node_profiles.get(&req.counterparty).cloned();

        let chat = Chat {
            id: chat_id.clone(),
            counterparty: req.counterparty.clone(),
            messages: Vec::new(),
            last_activity: timestamp,
            unread_count: 0,
            is_blocked: false,
            notify: true,
            counterparty_profile,
        };

        self.chats.insert(chat_id.clone(), chat.clone());
        self.rebuild_chat_search(&chat_id);

        // Notify the counterparty about the chat creation and our profile asynchronously
        let target = Address::from((req.counterparty.as_str(), OUR_PROCESS_ID));
        let our_node = our().node.clone();
        let our_profile = self.profile.clone();

        // Spawn task to notify counterparty without blocking
        spawn(async move {
            // First notify about chat creation
            match receive_chat_creation_remote_rpc(&target, our_node.clone()).await {
                Ok(_) => log_debug!("Successfully notified counterparty about chat creation"),
                Err(e) => log_debug!("Failed to notify counterparty about chat creation: {:?}", e),
            }

            // Then share our profile
            let cu_profile = ChatState::to_cu_user_profile(&our_profile);
            match receive_profile_update_remote_rpc(&target, our_node, cu_profile).await {
                Ok(_) => log_debug!("Successfully shared profile with counterparty"),
                Err(e) => log_debug!("Failed to share profile with counterparty: {:?}", e),
            }
        });

        Ok(chat)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn get_chats(&self) -> Result<Vec<Chat>, String> {
        let mut chats: Vec<Chat> = self.chats.values().cloned().collect();
        log_debug!("get_chats: Returning {} chats", chats.len());
        for chat in &chats {
            log_debug!("  Chat: {} with {}", chat.id, chat.counterparty);
        }
        chats.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        Ok(chats)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn get_chat(&self, req: GetChatReq) -> Result<Chat, String> {
        self.chats
            .get(&req.chat_id)
            .cloned()
            .ok_or_else(|| "Chat not found".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn get_messages(&self, req: GetMessagesReq) -> Result<Vec<ChatMessage>, String> {
        // Get the chat
        let chat = self
            .chats
            .get(&req.chat_id)
            .ok_or_else(|| "Chat not found".to_string())?;

        // Sort by timestamp descending (newest first) and break ties via sequence/id
        let mut messages: Vec<ChatMessage> = chat.messages.clone();
        messages.sort_by(|a, b| match b.timestamp.cmp(&a.timestamp) {
            Ordering::Equal => match (
                b.sequence.unwrap_or(0).cmp(&a.sequence.unwrap_or(0)),
                b.id.cmp(&a.id),
            ) {
                (Ordering::Equal, id_cmp) => id_cmp,
                (seq_cmp, _) => seq_cmp,
            },
            other => other,
        });

        let limit = req.limit.unwrap_or(50) as usize;
        if let Some(before_ts) = req.before_timestamp {
            let newer_count = messages
                .iter()
                .filter(|msg| msg.timestamp > before_ts)
                .count();
            let mut to_skip_at_ts = limit.saturating_sub(newer_count);
            messages = messages
                .into_iter()
                .filter(|msg| {
                    if msg.timestamp > before_ts {
                        false
                    } else if msg.timestamp < before_ts {
                        true
                    } else if to_skip_at_ts > 0 {
                        to_skip_at_ts -= 1;
                        false
                    } else {
                        true
                    }
                })
                .collect();
        }

        // Apply limit (convert u64 to usize for truncate)
        messages.truncate(limit);

        // Return in ascending order (oldest first) for display
        messages.reverse();

        Ok(messages)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_sync_hash(&self, req: GetSyncHashReq) -> Result<SyncHashInfo, String> {
        let chat = self
            .chats
            .get(&req.chat_id)
            .ok_or_else(|| "Chat not found".to_string())?;

        // Calculate a hash of the message history
        let mut hasher = DefaultHasher::new();

        // Hash message count
        chat.messages.len().hash(&mut hasher);

        // Hash each message's key fields (id, sender, content, timestamp)
        for msg in &chat.messages {
            msg.id.hash(&mut hasher);
            msg.sender.hash(&mut hasher);
            msg.content.hash(&mut hasher);
            msg.timestamp.hash(&mut hasher);

            // Also hash reactions to detect reaction desyncs
            for reaction in &msg.reactions {
                reaction.emoji.hash(&mut hasher);
                reaction.user.hash(&mut hasher);
                reaction.timestamp.hash(&mut hasher);
            }
        }

        let hash = hasher.finish();

        Ok(SyncHashInfo {
            chat_id: req.chat_id,
            message_count: chat.messages.len() as u32,
            last_message_id: chat.messages.last().map(|m| m.id.clone()),
            last_message_timestamp: chat.messages.last().map(|m| m.timestamp),
            hash: format!("{:x}", hash),
        })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_all_sync_hashes(&self) -> Result<Vec<SyncHashInfo>, String> {
        let mut sync_hashes = Vec::new();

        for (chat_id, chat) in &self.chats {
            let mut hasher = DefaultHasher::new();

            // Hash message count
            chat.messages.len().hash(&mut hasher);

            // Hash each message's key fields
            for msg in &chat.messages {
                msg.id.hash(&mut hasher);
                msg.sender.hash(&mut hasher);
                msg.content.hash(&mut hasher);
                msg.timestamp.hash(&mut hasher);

                // Also hash reactions
                for reaction in &msg.reactions {
                    reaction.emoji.hash(&mut hasher);
                    reaction.user.hash(&mut hasher);
                    reaction.timestamp.hash(&mut hasher);
                }
            }

            let hash = hasher.finish();

            sync_hashes.push(SyncHashInfo {
                chat_id: chat_id.clone(),
                message_count: chat.messages.len() as u32,
                last_message_id: chat.messages.last().map(|m| m.id.clone()),
                last_message_timestamp: chat.messages.last().map(|m| m.timestamp),
                hash: format!("{:x}", hash),
            });
        }

        Ok(sync_hashes)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn delete_chat(&mut self, req: DeleteChatReq) -> Result<String, String> {
        self.chats
            .remove(&req.chat_id)
            .ok_or_else(|| "Chat not found".to_string())?;
        self.message_sequence_counters.remove(&req.chat_id);
        self.rebuild_chat_search(&req.chat_id);
        Ok("Chat deleted".to_string())
    }

    #[local]
    #[http]
    async fn update_chat_settings(&mut self, req: UpdateChatSettingsReq) -> Result<Chat, String> {
        let chat = self
            .chats
            .get_mut(&req.chat_id)
            .ok_or_else(|| "Chat not found".to_string())?;

        if let Some(notify) = req.notify {
            chat.notify = notify;
        }

        Ok(chat.clone())
    }

    // GROUP OPERATIONS

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn create_group(&mut self, req: CreateGroupReq) -> Result<CreateGroupRes, String> {
        self.create_group_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn list_groups(&self) -> Result<ListGroupsRes, String> {
        Ok(self.list_groups_state())
    }

    #[local]
    #[http]
    async fn update_group_settings(
        &mut self,
        req: UpdateGroupSettingsReq,
    ) -> Result<GroupSummary, String> {
        // Verify group exists
        let group = self
            .groups
            .get(&req.group_id)
            .ok_or_else(|| "Group not found".to_string())?;

        if let Some(notify) = req.notify {
            self.group_notify.insert(req.group_id.clone(), notify);
        }

        Ok(GroupSummary {
            group_id: req.group_id.clone(),
            metadata: group.metadata.clone(),
            member_count: group.members.len(),
            thread_count: group.threads.len(),
            unread_count: self.group_unread.get(&req.group_id).copied().unwrap_or(0),
            notify: self
                .group_notify
                .get(&req.group_id)
                .copied()
                .unwrap_or(true),
        })
    }

    #[http]
    async fn create_group_join_link(
        &mut self,
        req: CreateGroupJoinLinkReq,
    ) -> Result<CreateGroupJoinLinkRes, String> {
        self.require_group_permission(&req.group_id, &our().node, GroupPermissions::INVITE_MEMBERS)
            .map_err(|err| format!("cannot create join link: {}", err))?;

        let visibility = self
            .groups
            .get(&req.group_id)
            .and_then(|group| group.metadata.as_ref().map(|meta| meta.visibility))
            .unwrap_or(GroupVisibility::Private);
        if visibility != GroupVisibility::Public {
            return Err("Group is not public".to_string());
        }

        for join_key in self.group_join_keys.values_mut() {
            if join_key.group_id == req.group_id {
                join_key.is_revoked = true;
            }
        }

        let key = format!("{:x}", rand::random::<u128>());
        let join_key = GroupJoinKey {
            key: key.clone(),
            group_id: req.group_id.clone(),
            created_at: current_timestamp(),
            is_revoked: false,
        };
        self.group_join_keys.insert(key.clone(), join_key);

        let link = format!(
            "hw://{}/join-group/{}/{}",
            our().package_id(),
            our().node,
            key
        );
        Ok(CreateGroupJoinLinkRes { link })
    }

    #[http]
    async fn join_group_link(&mut self, req: JoinGroupLinkReq) -> Result<JoinGroupLinkRes, String> {
        if req.host == our().node {
            return self.join_group_link_internal(req.key, our().node.clone());
        }

        let remote_req = JoinGroupLinkRemoteReq { key: req.key };
        let body = serde_json::to_vec(&serde_json::json!({ "JoinGroupLinkRemote": remote_req }))
            .map_err(|e| format!("Failed to encode join link request: {:?}", e))?;
        let target = Address::from((req.host.as_str(), OUR_PROCESS_ID));

        match send::<Result<JoinGroupLinkRes, String>>(Request::to(&target).body(body)).await {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(err)) => Err(err),
            Err(err) => Err(format!("Failed to join group: {:?}", err)),
        }
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_group(&self, req: GetGroupReq) -> Result<GetGroupRes, String> {
        // Check if caller is a member of the group (Active or Removed)
        // Removed members can still view the group to see their removal status
        let group = self
            .groups
            .get(&req.group_id)
            .ok_or_else(|| "group not found".to_string())?;
        let caller = our().node;
        let member = group
            .members
            .get(&caller)
            .ok_or_else(|| format!("{} is not a member of group {}", caller, req.group_id))?;
        // Allow Active and Removed members to view the group
        // Only reject Pending members (they haven't been approved yet)
        if member.status == crate::crdt::MembershipStatus::Pending {
            return Err(format!(
                "member {} is pending in group {} and cannot view it yet",
                caller, req.group_id
            ));
        }
        Ok(self.get_group_state(req))
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn create_group_thread(
        &mut self,
        req: CreateGroupThreadReq,
    ) -> Result<CreateGroupThreadRes, String> {
        self.create_group_thread_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn send_group_message(
        &mut self,
        req: SendGroupMessageReq,
    ) -> Result<SendGroupMessageRes, String> {
        self.send_group_message_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn edit_group_message(
        &mut self,
        req: EditGroupMessageReq,
    ) -> Result<SendGroupMessageRes, String> {
        self.edit_group_message_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn delete_group_message(&mut self, req: DeleteGroupMessageReq) -> Result<String, String> {
        self.delete_group_message_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn add_group_reaction(&mut self, req: AddGroupReactionReq) -> Result<String, String> {
        self.add_group_reaction_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn remove_group_reaction(
        &mut self,
        req: RemoveGroupReactionReq,
    ) -> Result<String, String> {
        self.remove_group_reaction_state(req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn invite_group_member(
        &mut self,
        req: InviteGroupMemberReq,
    ) -> Result<MembershipDecisionRes, String> {
        let candidate = req.candidate.clone();
        let group_id = req.group_id.clone();
        let decision = self
            .invite_member(
                &req.group_id,
                our().node.clone(),
                req.candidate,
                req.role_id,
            )
            .map_err(|err| err.to_string())?;

        // If the invite was approved, immediately push a snapshot to the new member
        // so they can bootstrap without waiting for the replication scheduler's debounce
        if decision.status == MembershipDecisionStatus::Approved {
            spawn_immediate_snapshot_push(group_id, candidate);
        }

        Ok(MembershipDecisionRes { decision })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn approve_group_membership(
        &mut self,
        req: ApproveGroupMembershipReq,
    ) -> Result<MembershipDecisionRes, String> {
        let decision = self
            .approve_membership(&req.group_id, &req.proposal_id, our().node.clone())
            .map_err(|err| err.to_string())?;
        Ok(MembershipDecisionRes { decision })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn remove_group_member(
        &mut self,
        req: RemoveGroupMemberReq,
    ) -> Result<MembershipDecisionRes, String> {
        let decision = self
            .remove_member(&req.group_id, our().node.clone(), req.member)
            .map_err(|err| err.to_string())?;
        Ok(MembershipDecisionRes { decision })
    }

    #[remote]
    async fn join_group_link_remote(
        &mut self,
        req: JoinGroupLinkRemoteReq,
    ) -> Result<JoinGroupLinkRes, String> {
        let caller = source().node.clone();
        let res = self.join_group_link_internal(req.key, caller.clone())?;
        spawn_immediate_snapshot_push(res.group_id.clone(), caller);
        Ok(res)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn send_message(&mut self, req: SendMessageReq) -> Result<ChatMessage, String> {
        self.send_message_internal(&req.chat_id, req.content, req.reply_to, None)
    }

    #[local]
    #[http]
    async fn record_payment(&mut self, req: RecordPaymentReq) -> Result<ChatMessage, String> {
        if !self.chats.contains_key(&req.chat_id) {
            return Err("Chat not found".to_string());
        }

        let tx_hash = req.tx_hash.trim().to_string();
        if !Self::is_valid_tx_hash(&tx_hash) {
            return Err("Invalid transaction hash".to_string());
        }

        let amount = req.amount.trim().to_string();
        if amount.is_empty() {
            return Err("Amount is required".to_string());
        }
        let parsed_amount = amount
            .parse::<f64>()
            .map_err(|_| "Amount must be a valid number".to_string())?;
        if parsed_amount <= 0.0 {
            return Err("Amount must be greater than zero".to_string());
        }

        let coin_name = if req.coin_name.trim().is_empty() {
            "ETH".to_string()
        } else {
            req.coin_name.trim().to_uppercase()
        };

        let from_address = req.from_address.trim().to_string();
        if !Self::is_valid_evm_address(&from_address) {
            return Err("Invalid sender address".to_string());
        }

        let to_address = req.to_address.trim().to_string();
        if !Self::is_valid_evm_address(&to_address) {
            return Err("Invalid recipient address".to_string());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message_id = format!("payment:{}:{}", timestamp, rand::random::<u32>());
        let explorer_url = format!("https://basescan.org/tx/{tx_hash}");

        let message = ChatMessage {
            id: message_id,
            sender: our().node.clone(),
            content: format!(
                "{} sent {} {} to {}",
                our().node,
                amount,
                coin_name,
                to_address
            ),
            timestamp,
            sequence: None,
            status: MessageStatus::Sending,
            reply_to: None,
            reactions: Vec::new(),
            message_type: MessageType::Payment,
            file_info: None,
            payment_info: Some(PaymentInfo {
                tx_hash,
                amount,
                coin_name,
                from_address,
                to_address,
                explorer_url,
            }),
        };

        let (counterparty, stored_message) =
            self.stage_outgoing_message(&req.chat_id, message, None);
        self.dispatch_outgoing_message(counterparty, stored_message.clone());

        Ok(self
            .chats
            .get(&req.chat_id)
            .and_then(|chat| {
                chat.messages
                    .iter()
                    .find(|m| m.id == stored_message.id)
                    .cloned()
            })
            .unwrap_or(stored_message))
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn edit_message(&mut self, req: EditMessageReq) -> Result<String, String> {
        let mut broadcast_update: Option<WsServerMessage> = None;
        let mut remote_edit: Option<(String, String, String, String)> = None;
        let mut needs_rebuild = false;

        if let Some(chat) = self.chats.get_mut(&req.chat_id) {
            if let Some(message) = chat.messages.iter_mut().find(|m| m.id == req.message_id) {
                if message.sender != our().node {
                    return Ok("Ignoring edit for remote message".to_string());
                }
                if message.message_type == MessageType::Payment {
                    return Err("Payment events cannot be edited".to_string());
                }
                message.content = req.new_content.clone();
                needs_rebuild = true;
                broadcast_update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                remote_edit = Some((
                    chat.counterparty.clone(),
                    req.chat_id.clone(),
                    req.message_id.clone(),
                    req.new_content.clone(),
                ));
            }
        }

        if needs_rebuild {
            self.rebuild_chat_search(&req.chat_id);
        }

        if let Some(update) = &broadcast_update {
            self.broadcast_ws_message(update);
        }

        if let Some((counterparty, chat_id, message_id, new_content)) = remote_edit {
            spawn(async move {
                let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
                match receive_message_edit_remote_rpc(&target, chat_id, message_id, new_content)
                    .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => log_debug!(
                        "Counterparty {} rejected message edit: {}",
                        counterparty,
                        err
                    ),
                    Err(err) => {
                        log_debug!("Failed to send message edit to {}: {:?}", counterparty, err)
                    }
                }
            });
        }

        if broadcast_update.is_some() {
            return Ok("Message edited".to_string());
        }

        Err("Message not found".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn delete_message(&mut self, req: DeleteMessageReq) -> Result<String, String> {
        let mut chat_update: Option<WsServerMessage> = None;
        let mut deletion_notice: Option<(String, String, String, bool)> = None;
        let mut needs_rebuild = false;

        if let Some(chat) = self.chats.get_mut(&req.chat_id) {
            if let Some(pos) = chat.messages.iter().position(|m| m.id == req.message_id) {
                if chat.messages[pos].message_type == MessageType::Payment {
                    return Err("Payment events cannot be deleted".to_string());
                }
                let counterparty = chat.counterparty.clone();
                let message_id = req.message_id.clone();
                let chat_id = req.chat_id.clone();
                let delete_for_both = req.delete_for_both.unwrap_or(false);

                chat.messages.remove(pos);
                needs_rebuild = true;

                chat_update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                deletion_notice = Some((counterparty, message_id, chat_id, delete_for_both));
            }
        }

        if needs_rebuild {
            self.rebuild_chat_search(&req.chat_id);
        }

        if let Some(update) = &chat_update {
            self.broadcast_ws_message(update);
        }

        if let Some((counterparty, message_id, chat_id, delete_for_both)) = deletion_notice {
            if delete_for_both {
                let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
                spawn(async move {
                    let _ = receive_message_deletion_remote_rpc(&target, message_id, chat_id).await;
                });
            }
            return Ok("Message deleted".to_string());
        }

        Err("Message not found".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn add_reaction(&mut self, req: AddReactionReq) -> Result<String, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let reaction = MessageReaction {
            emoji: req.emoji.clone(),
            user: our().node.clone(),
            timestamp,
        };

        let mut addition: Option<(WsServerMessage, String, String, String)> = None;

        // Find and add reaction to message in the specified chat
        if let Some(chat) = self.chats.get_mut(&req.chat_id) {
            if let Some(message) = chat.messages.iter_mut().find(|m| m.id == req.message_id) {
                // Check if user already reacted with this emoji
                if !message
                    .reactions
                    .iter()
                    .any(|r| r.user == reaction.user && r.emoji == reaction.emoji)
                {
                    message.reactions.push(reaction.clone());

                    let target_node = if message.sender != our().node {
                        message.sender.clone()
                    } else {
                        chat.counterparty.clone()
                    };

                    addition = Some((
                        WsServerMessage::ChatUpdate(chat.clone()),
                        target_node,
                        req.message_id.clone(),
                        req.emoji.clone(),
                    ));
                } else {
                    return Ok("Already reacted".to_string());
                }
            }
        }

        if let Some((chat_update, target_node, msg_id, emoji)) = addition {
            self.broadcast_ws_message(&chat_update);
            let user = our().node.clone();
            spawn(async move {
                let target = Address::new(&target_node, OUR_PROCESS_ID.clone());
                match receive_reaction_remote_rpc(&target, msg_id, emoji, user).await {
                    Ok(_) => log_debug!("Successfully sent reaction to counterparty"),
                    Err(e) => log_debug!("Failed to send reaction to counterparty: {:?}", e),
                }
            });
            return Ok("Reaction added".to_string());
        }

        Err("Message not found".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn forward_message(&mut self, req: ForwardMessageReq) -> Result<ChatMessage, String> {
        // Find the message to forward from the specified chat
        let message_to_forward = self
            .chats
            .get(&req.from_chat_id)
            .and_then(|chat| chat.messages.iter().find(|m| m.id == req.message_id))
            .cloned();

        let original_message = message_to_forward.ok_or_else(|| "Message not found".to_string())?;
        if original_message.message_type == MessageType::Payment {
            return Err("Payment events cannot be forwarded".to_string());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let chat_id = req.to_chat_id.clone();

        let mut forwarded_message = ChatMessage {
            id: format!("{}:{}", timestamp, rand::random::<u32>()),
            sender: our().node.clone(),
            content: format!("Forwarded: {}", original_message.content),
            timestamp,
            sequence: None,
            status: MessageStatus::Sending,
            reply_to: None,
            reactions: Vec::new(),
            message_type: original_message.message_type.clone(),
            file_info: original_message.file_info.clone(),
            payment_info: original_message.payment_info.clone(),
        };

        self.assign_sequence_to_message(&chat_id, &mut forwarded_message);

        let (counterparty, chat_snapshot) = {
            let chat = self.get_or_create_chat(&chat_id, timestamp, None, None);
            chat.messages.push(forwarded_message.clone());
            chat.last_activity = timestamp;
            (chat.counterparty.clone(), chat.clone())
        };
        self.rebuild_chat_search(&chat_id);

        // Send to counterparty if it's a node-to-node chat
        if !chat_id.starts_with("browser:") {
            let msg_to_send = forwarded_message.clone();

            let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));

            // Send using generated RPC method
            let msg_json = serde_json::to_value(&msg_to_send).unwrap();
            let msg_for_rpc: CUChatMessage = serde_json::from_value(msg_json).unwrap();
            match receive_message_remote_rpc(&target, msg_for_rpc).await {
                Ok(_) => {
                    if let Some(chat) = self.chats.get_mut(&req.to_chat_id) {
                        if let Some(msg) = chat
                            .messages
                            .iter_mut()
                            .find(|m| m.id == forwarded_message.id)
                        {
                            msg.status =
                                safe_update_message_status(&msg.status, MessageStatus::Sent);
                        }

                        // Send ChatUpdate with the updated message status
                        let chat_update = WsServerMessage::ChatUpdate(chat_snapshot.clone());
                        self.broadcast_ws_message(&chat_update);
                    }
                }
                Err(_) => {
                    self.enqueue_delivery_message(&counterparty, msg_to_send);
                    if let Some(chat) = self.chats.get_mut(&req.to_chat_id) {
                        if let Some(msg) = chat
                            .messages
                            .iter_mut()
                            .find(|m| m.id == forwarded_message.id)
                        {
                            msg.status =
                                safe_update_message_status(&msg.status, MessageStatus::Failed);
                        }
                    }
                }
            }
        }
        Ok(forwarded_message)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn remove_reaction(&mut self, req: RemoveReactionReq) -> Result<String, String> {
        let user = our().node.clone();
        let mut removal: Option<(WsServerMessage, String, String, String)> = None;

        // Find and remove reaction from message, and determine counterparty to notify
        if let Some(chat) = self.chats.get_mut(&req.chat_id) {
            if let Some(message) = chat.messages.iter_mut().find(|m| m.id == req.message_id) {
                if let Some(pos) = message
                    .reactions
                    .iter()
                    .position(|r| r.user == user && r.emoji == req.emoji)
                {
                    message.reactions.remove(pos);

                    let target_node = if message.sender != our().node {
                        message.sender.clone()
                    } else {
                        chat.counterparty.clone()
                    };

                    removal = Some((
                        WsServerMessage::ChatUpdate(chat.clone()),
                        target_node,
                        req.message_id.clone(),
                        req.emoji.clone(),
                    ));
                }
            }
        }

        if let Some((chat_update, target_node, msg_id, emoji)) = removal {
            // Update local subscribers
            self.broadcast_ws_message(&chat_update);

            // Notify counterparty to remove the reaction on their copy as well
            spawn(async move {
                let target = Address::new(&target_node, OUR_PROCESS_ID.clone());
                match receive_reaction_remove_remote_rpc(&target, msg_id, emoji, user).await {
                    Ok(_) => log_debug!("Successfully sent reaction removal to counterparty"),
                    Err(e) => {
                        log_debug!("Failed to send reaction removal to counterparty: {:?}", e)
                    }
                }
            });

            return Ok("Reaction removed".to_string());
        }

        Err("Reaction not found".to_string())
    }

    // BROWSER CHAT MANAGEMENT

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn create_chat_link(&mut self, req: CreateChatLinkReq) -> Result<String, String> {
        let key = format!("{:x}", rand::random::<u128>());
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let chat_key = ChatKey {
            key: key.clone(),
            user_name: format!("Guest-{}", rand::random::<u32>() % 10000),
            created_at: timestamp,
            is_revoked: false,
            chat_id: req.chat_id.clone(),
        };

        self.chat_keys.insert(key.clone(), chat_key);

        let link = format!("http://{}/public/join-{}", our().node, key);
        Ok(link)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_chat_keys(&self) -> Result<Vec<ChatKey>, String> {
        Ok(self
            .chat_keys
            .values()
            .filter(|k| !k.is_revoked)
            .cloned()
            .collect())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn revoke_chat_key(&mut self, req: RevokeChatKeyReq) -> Result<String, String> {
        if let Some(key) = self.chat_keys.get_mut(&req.key) {
            key.is_revoked = true;
        } else {
            return Err("Chat key not found".to_string());
        }
        Ok("Chat key revoked".to_string())
    }

    // SETTINGS

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_settings(&self) -> Result<Settings, String> {
        Ok(self.settings.clone())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn update_settings(&mut self, settings: Settings) -> Result<String, String> {
        self.settings = settings;
        Ok("Settings updated".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn update_profile(&mut self, mut profile: UserProfile) -> Result<String, String> {
        let nickname = profile.name.trim();
        if nickname.is_empty() {
            profile.name = our().node.split('.').next().unwrap_or("User").to_string();
        } else {
            profile.name = nickname.to_string();
        }

        profile.base_address = profile.base_address.and_then(|addr| {
            let trimmed = addr.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

        if let Some(address) = profile.base_address.as_ref() {
            if !Self::is_valid_evm_address(address) {
                return Err("Base address must be a valid 0x Ethereum address".to_string());
            }
        }

        self.profile = profile.clone();

        // Notify all chat counterparties about the profile update
        let our_node = our().node.clone();
        let counterparties: Vec<String> = self
            .chats
            .values()
            .map(|chat| chat.counterparty.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for counterparty in counterparties {
            let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
            let node = our_node.clone();
            let prof = profile.clone();

            spawn(async move {
                let cu_profile = ChatState::to_cu_user_profile(&prof);
                match receive_profile_update_remote_rpc(&target, node, cu_profile).await {
                    Ok(_) => {
                        // Successfully notified counterparty
                    }
                    Err(_) => {
                        // Counterparty is likely offline, profile will be shared when they come online
                        // No need to print errors as this is expected behavior
                    }
                }
            });
        }

        Ok("Profile updated".to_string())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn upload_profile_picture(
        &mut self,
        req: UploadProfilePictureReq,
    ) -> Result<String, String> {
        // Validate mime type
        if !req.mime_type.starts_with("image/") {
            return Err("Invalid image type".to_string());
        }

        // Store the image data as a data URL
        let data_url = format!("data:{};base64,{}", req.mime_type, req.data);
        self.profile.profile_pic = Some(data_url.clone());

        // Notify all WebSocket connections about profile update
        let profile_update = WsServerMessage::ProfileUpdate {
            node: our().node.clone(),
            profile: self.profile.clone(),
        };
        self.broadcast_ws_message(&profile_update);

        // Notify all chat counterparties about the profile update
        let our_node = our().node.clone();
        let counterparties: Vec<String> = self
            .chats
            .values()
            .map(|chat| chat.counterparty.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for counterparty in counterparties {
            let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
            let node = our_node.clone();
            let prof = self.profile.clone();

            spawn(async move {
                let cu_profile = ChatState::to_cu_user_profile(&prof);
                match receive_profile_update_remote_rpc(&target, node, cu_profile).await {
                    Ok(_) => log_debug!("Notified {} about profile pic update", counterparty),
                    Err(e) => log_debug!(
                        "Failed to notify {} about profile pic update: {:?}",
                        counterparty,
                        e
                    ),
                }
            });
        }

        Ok(data_url)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn get_profile(&self) -> Result<UserProfile, String> {
        Ok(self.profile.clone())
    }

    // FILE AND VOICE NOTE OPERATIONS

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn upload_file(&mut self, req: UploadFileReq) -> Result<ChatMessage, String> {
        // Decode base64 data
        let file_data =
            base64_decode(&req.data).map_err(|e| format!("Failed to decode base64: {}", e))?;

        // Check file size limit
        let file_size_mb = (file_data.len() as u64) / (1024 * 1024);
        if file_size_mb > self.settings.max_file_size_mb {
            return Err(format!(
                "File size exceeds limit of {} MB",
                self.settings.max_file_size_mb
            ));
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let message_id = format!("{}:{}", timestamp, rand::random::<u32>());

        // Determine message type based on mime type
        let message_type = if req.mime_type.starts_with("image/") {
            MessageType::Image
        } else {
            MessageType::File
        };

        // Store file in VFS
        let package_id = our().package_id();
        let _safe_filename = req.filename.replace("/", "_").replace("..", "_");
        let file_id = format!("{}_{}", timestamp, rand::random::<u32>());
        let vfs_path = format!(
            "/{}/files/{}/{}",
            package_id,
            req.chat_id.replace(":", "_"),
            file_id
        );

        // Create directory if it doesn't exist
        let dir_path = format!("/{}/files/{}", package_id, req.chat_id.replace(":", "_"));
        let _ = vfs::open_dir(&dir_path, true, Some(5));

        // Create and write original file to VFS
        let file = vfs::create_file(&vfs_path, Some(5))
            .map_err(|e| format!("Failed to create VFS file: {:?}", e))?;
        file.write(&file_data)
            .map_err(|e| format!("Failed to write to VFS: {:?}", e))?;

        // For images, use data URL (they're usually small enough)
        // For other files, compress and send, or provide download link
        let (file_url, compressed_data) = if message_type == MessageType::Image {
            // Images: use data URL for easy inline display
            (format!("data:{};base64,{}", req.mime_type, req.data), None)
        } else {
            // Files: compress and prepare for sending
            let compressed = compress_data(&file_data)?;
            let compressed_b64 = base64_encode(&compressed);

            // Store compressed data for sending to counterparty
            // But locally, we'll serve from VFS
            let local_url = format!("/files/{}/{}", req.chat_id.replace(":", "_"), file_id);
            (local_url, Some(compressed_b64))
        };

        let file_info = FileInfo {
            filename: req.filename.clone(),
            mime_type: req.mime_type.clone(),
            size: file_data.len() as u64,
            url: file_url.clone(),
        };

        let chat_id = req.chat_id.clone();

        let message = ChatMessage {
            id: message_id,
            sender: our().node.clone(),
            content: req.filename,
            timestamp,
            sequence: None,
            status: MessageStatus::Sending,
            reply_to: req.reply_to,
            reactions: Vec::new(),
            message_type: message_type.clone(),
            file_info: Some(file_info),
            payment_info: None,
        };

        let (counterparty, stored_message) = self.stage_outgoing_message(&chat_id, message, None);

        let mut remote_message = stored_message.clone();
        if message_type == MessageType::File {
            if let Some(info) = remote_message.file_info.as_mut() {
                if let Some(compressed) = compressed_data {
                    info.url = format!("compressed:{}", compressed);
                }
            }
        }

        self.dispatch_outgoing_message(counterparty, remote_message);
        Ok(stored_message)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http(method = "POST", path = "/api/download-file")]
    async fn download_file(&self, req: DownloadFileReq) -> Result<Vec<u8>, String> {
        if req.chat_id.contains('/') || req.chat_id.contains("..") || req.file_id.contains('/') {
            set_response_status(hyperware_process_lib::http::StatusCode::BAD_REQUEST);
            return Err("Invalid file path".to_string());
        }

        let caller = source().node.clone();
        let chat = self.chats.get(&req.chat_id).or_else(|| {
            self.chats
                .values()
                .find(|chat| chat.id.replace(":", "_") == req.chat_id)
        });
        let chat = chat.ok_or_else(|| {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            "Chat not found".to_string()
        })?;
        if caller != our().node && caller != chat.counterparty {
            set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
            return Err("unauthorized".to_string());
        }

        let chat_id = chat.id.replace(":", "_");
        let package_id = our().package_id();
        let vfs_path = format!("/{}/files/{}/{}", package_id, chat_id, req.file_id);

        let file = vfs::open_file(&vfs_path, false, Some(5)).map_err(|e| {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            format!("Failed to open file: {:?}", e)
        })?;

        let file_data = file.read().map_err(|e| {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            format!("Failed to read file: {:?}", e)
        })?;

        Ok(file_data)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn upload_group_file(
        &mut self,
        req: UploadGroupFileReq,
    ) -> Result<SendGroupMessageRes, String> {
        self.require_group_permission(&req.group_id, &our().node, GroupPermissions::SEND_MESSAGES)
            .map_err(|err| {
                set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
                format!("unauthorized: {}", err)
            })?;
        self.require_subscriber_access(&req.group_id, &our().node)
            .map_err(|err| {
                set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
                format!("unauthorized: {}", err)
            })?;

        let file_data =
            base64_decode(&req.data).map_err(|e| format!("Failed to decode base64: {}", e))?;

        let file_size_mb = (file_data.len() as u64) / (1024 * 1024);
        if file_size_mb > self.settings.max_file_size_mb {
            return Err(format!(
                "File size exceeds limit of {} MB",
                self.settings.max_file_size_mb
            ));
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let message_type = if req.mime_type.starts_with("image/") {
            MessageType::Image
        } else {
            MessageType::File
        };

        let package_id = our().package_id();
        let file_id = format!("{}_{}", timestamp, rand::random::<u32>());
        let group_dir = req.group_id.replace(":", "_");
        let file_url = format!("/files/{}/{}", group_dir, file_id);
        let vfs_path = format!("/{}/files/{}/{}", package_id, group_dir, file_id);

        let dir_path = format!("/{}/files/{}", package_id, group_dir);
        let _ = vfs::open_dir(&dir_path, true, Some(5));

        let file = vfs::create_file(&vfs_path, Some(5))
            .map_err(|e| format!("Failed to create VFS file: {:?}", e))?;
        file.write(&file_data)
            .map_err(|e| format!("Failed to write to VFS: {:?}", e))?;

        let attachment = crate::crdt::AttachmentDescriptor {
            attachment_id: file_id,
            filename: req.filename.clone(),
            mime_type: req.mime_type.clone(),
            size_bytes: file_data.len() as u64,
            checksum: None,
            uri: Some(file_url),
        };

        let send_req = SendGroupMessageReq {
            group_id: req.group_id,
            thread_id: req.thread_id,
            content: req.filename,
            message_type,
            reply_to: req.reply_to,
            attachments: vec![attachment],
        };

        self.send_group_message_state(send_req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn send_group_voice_note(
        &mut self,
        req: SendGroupVoiceNoteReq,
    ) -> Result<SendGroupMessageRes, String> {
        self.require_group_permission(&req.group_id, &our().node, GroupPermissions::SEND_MESSAGES)
            .map_err(|err| {
                set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
                format!("unauthorized: {}", err)
            })?;
        self.require_subscriber_access(&req.group_id, &our().node)
            .map_err(|err| {
                set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
                format!("unauthorized: {}", err)
            })?;

        let file_data = base64_decode(&req.audio_data)
            .map_err(|e| format!("Failed to decode base64: {}", e))?;

        let file_size_mb = (file_data.len() as u64) / (1024 * 1024);
        if file_size_mb > self.settings.max_file_size_mb {
            return Err(format!(
                "File size exceeds limit of {} MB",
                self.settings.max_file_size_mb
            ));
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let message_type = MessageType::VoiceNote;
        let package_id = our().package_id();
        let file_id = format!("{}_{}", timestamp, rand::random::<u32>());
        let group_dir = req.group_id.replace(":", "_");
        let file_url = format!("/files/{}/{}", group_dir, file_id);
        let vfs_path = format!("/{}/files/{}/{}", package_id, group_dir, file_id);

        let dir_path = format!("/{}/files/{}", package_id, group_dir);
        let _ = vfs::open_dir(&dir_path, true, Some(5));

        let file = vfs::create_file(&vfs_path, Some(5))
            .map_err(|e| format!("Failed to create VFS file: {:?}", e))?;
        file.write(&file_data)
            .map_err(|e| format!("Failed to write to VFS: {:?}", e))?;

        let extension = req
            .mime_type
            .split('/')
            .nth(1)
            .and_then(|ext| ext.split(';').next())
            .unwrap_or("webm");
        let filename = format!("voice_note_{}.{}", timestamp, extension);

        let attachment = crate::crdt::AttachmentDescriptor {
            attachment_id: file_id,
            filename,
            mime_type: req.mime_type.clone(),
            size_bytes: file_data.len() as u64,
            checksum: None,
            uri: Some(file_url),
        };

        let content = format!("Voice note ({}s)", req.duration);
        let send_req = SendGroupMessageReq {
            group_id: req.group_id,
            thread_id: req.thread_id,
            content,
            message_type,
            reply_to: req.reply_to,
            attachments: vec![attachment],
        };

        self.send_group_message_state(send_req)
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[http(method = "POST", path = "/api/download-group-file")]
    async fn download_group_file(&mut self, req: DownloadGroupFileReq) -> Result<Vec<u8>, String> {
        if req.group_id.contains('/')
            || req.group_id.contains("..")
            || req.attachment_id.contains('/')
        {
            set_response_status(hyperware_process_lib::http::StatusCode::BAD_REQUEST);
            return Err("Invalid file path".to_string());
        }

        let caller = source().node.clone();
        self.require_subscriber_access(&req.group_id, &caller)
            .map_err(|err| {
                set_response_status(hyperware_process_lib::http::StatusCode::FORBIDDEN);
                format!("unauthorized: {}", err)
            })?;

        let group_dir = req.group_id.replace(":", "_");
        let package_id = our().package_id();
        let vfs_path = format!("/{}/files/{}/{}", package_id, group_dir, req.attachment_id);

        if let Ok(file) = vfs::open_file(&vfs_path, false, Some(5)) {
            let file_data = file.read().map_err(|e| {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                format!("Failed to read file: {:?}", e)
            })?;
            return Ok(file_data);
        }

        let sender = self
            .groups
            .get(&req.group_id)
            .and_then(|group| {
                group.messages.values().find_map(|meta| {
                    meta.attachments
                        .iter()
                        .any(|att| att.attachment_id == req.attachment_id)
                        .then(|| meta.sender.clone())
                })
            })
            .ok_or_else(|| {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                "Attachment not found".to_string()
            })?;

        if sender == our().node {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            return Err("Attachment not available".to_string());
        }

        let fetch_req = FetchGroupFileReq {
            group_id: req.group_id.clone(),
            attachment_id: req.attachment_id.clone(),
        };
        let body = serde_json::to_vec(&serde_json::json!({ "FetchGroupFile": fetch_req }))
            .map_err(|e| format!("Failed to encode fetch request: {:?}", e))?;
        let target = Address::from((sender.as_str(), OUR_PROCESS_ID));

        match send::<Result<Vec<u8>, String>>(Request::to(&target).body(body)).await {
            Ok(Ok(file_data)) => {
                let dir_path = format!("/{}/files/{}", package_id, group_dir);
                let _ = vfs::open_dir(&dir_path, true, Some(5));
                if let Ok(file) = vfs::create_file(&vfs_path, Some(5)) {
                    let _ = file.write(&file_data);
                }
                Ok(file_data)
            }
            Ok(Err(err)) => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                Err(err)
            }
            Err(err) => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                Err(format!("Failed to fetch attachment: {:?}", err))
            }
        }
    }

    #[remote]
    async fn fetch_group_file(&self, req: FetchGroupFileReq) -> Result<Vec<u8>, String> {
        let caller = source().node.clone();
        self.require_subscriber_access(&req.group_id, &caller)
            .map_err(|err| format!("unauthorized: {}", err))?;

        if req.group_id.contains('/')
            || req.group_id.contains("..")
            || req.attachment_id.contains('/')
        {
            return Err("Invalid file path".to_string());
        }

        let group_dir = req.group_id.replace(":", "_");
        let package_id = our().package_id();
        let vfs_path = format!("/{}/files/{}/{}", package_id, group_dir, req.attachment_id);

        let file = vfs::open_file(&vfs_path, false, Some(5))
            .map_err(|e| format!("Failed to open file: {:?}", e))?;
        let file_data = file
            .read()
            .map_err(|e| format!("Failed to read file: {:?}", e))?;
        Ok(file_data)
    }
    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn send_voice_note(&mut self, req: SendVoiceNoteReq) -> Result<ChatMessage, String> {
        let audio_bytes = base64_decode(&req.audio_data).map_err(|e| {
            set_response_status(hyperware_process_lib::http::StatusCode::BAD_REQUEST);
            format!("Failed to decode base64: {}", e)
        })?;
        let audio_size_mb = (audio_bytes.len() as u64) / (1024 * 1024);
        if audio_size_mb > self.settings.max_file_size_mb {
            set_response_status(hyperware_process_lib::http::StatusCode::PAYLOAD_TOO_LARGE);
            return Err(format!(
                "File size exceeds limit of {} MB",
                self.settings.max_file_size_mb
            ));
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let message_id = format!("{}:{}", timestamp, rand::random::<u32>());

        // Store voice note
        let file_url = format!("data:audio/webm;base64,{}", req.audio_data);

        let file_info = FileInfo {
            filename: format!("voice_note_{}.webm", message_id),
            mime_type: "audio/webm".to_string(),
            size: audio_bytes.len() as u64,
            url: file_url,
        };

        let chat_id = req.chat_id.clone();

        let message = ChatMessage {
            id: message_id,
            sender: our().node.clone(),
            content: format!("Voice note ({}s)", req.duration),
            timestamp,
            sequence: None,
            status: MessageStatus::Sending,
            reply_to: req.reply_to,
            reactions: Vec::new(),
            message_type: MessageType::VoiceNote,
            file_info: Some(file_info),
            payment_info: None,
        };

        let (counterparty, stored_message) = self.stage_outgoing_message(&chat_id, message, None);
        self.dispatch_outgoing_message(counterparty, stored_message.clone());
        Ok(stored_message)
    }

    // P2P MESSAGE RECEIVING

    #[remote]
    async fn receive_chat_creation(&mut self, mut counterparty: String) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            if counterparty != caller_node {
                log_debug!(
                    "[SEC] receive_chat_creation rejected spoofed counterparty={} source={}",
                    counterparty,
                    caller_node
                );
                return Err("receive_chat_creation rejected spoofed counterparty".to_string());
            }
            counterparty = caller_node;
        }
        log_debug!("receive_chat_creation: Got request from {}", counterparty);

        // Normalize chat ID to always be alphabetically sorted
        let chat_id = Self::normalize_chat_id(&counterparty, &our().node);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.reconcile_dm_chats_for_counterparty(&counterparty);

        // Check if chat already exists
        let chat_exists = self.chats.contains_key(&chat_id);
        let mut created_chat = false;
        if !chat_exists {
            // Get counterparty profile if we have it
            let counterparty_profile = self.node_profiles.get(&counterparty).cloned();

            let chat = Chat {
                id: chat_id.clone(),
                counterparty: counterparty.clone(),
                messages: Vec::new(),
                last_activity: timestamp,
                unread_count: 0,
                is_blocked: false,
                notify: true,
                counterparty_profile,
            };

            self.chats.insert(chat_id.clone(), chat.clone());
            self.rebuild_chat_search(&chat_id);
            log_debug!("receive_chat_creation: Created chat {}", chat_id);
            created_chat = true;

            // Notify WebSocket connections about the new chat
            log_debug!(
                "receive_chat_creation: WebSocket connections: {}",
                self.ws_connections.len()
            );
            let chat_update = WsServerMessage::ChatUpdate(chat.clone());
            self.broadcast_ws_message(&chat_update);
        } else {
            log_debug!("receive_chat_creation: Chat {} already exists", chat_id);
        }

        if created_chat {}

        // Signal the delivery worker (step 3) to flush anything pending to this node
        self.enqueue_delivery_flush(&counterparty);

        // Share our profile with the counterparty
        let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
        let our_node = our().node.clone();
        let our_profile = self.profile.clone();

        spawn(async move {
            let cu_profile = ChatState::to_cu_user_profile(&our_profile);
            match receive_profile_update_remote_rpc(&target, our_node, cu_profile).await {
                Ok(_) => {
                    // Successfully shared profile
                }
                Err(_) => {
                    // Counterparty is likely offline, profile will be shared when they come online
                }
            }
        });

        Ok(())
    }

    #[remote]
    async fn receive_message(&mut self, mut message: ChatMessage) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            if message.sender != caller_node {
                log_debug!(
                    "[SEC] receive_message rejected spoofed sender={} source={}",
                    message.sender,
                    caller_node
                );
                return Err("receive_message rejected spoofed sender".to_string());
            }
            message.sender = caller_node;
        }
        self.reconcile_dm_chats_for_counterparty(&message.sender);
        // Find or create chat for this message - normalize the ID
        let chat_id = Self::normalize_chat_id(&message.sender, &our().node);
        let is_new_chat = !self.chats.contains_key(&chat_id);
        let mut state_changed = false;

        self.chats.entry(chat_id.clone()).or_insert_with(|| Chat {
            id: chat_id.clone(),
            counterparty: message.sender.clone(),
            messages: Vec::new(),
            last_activity: message.timestamp,
            unread_count: 0,
            is_blocked: false,
            notify: true,
            counterparty_profile: self.node_profiles.get(&message.sender).cloned(),
        });
        if is_new_chat {
            state_changed = true;
        }

        // Update message status to Delivered
        let mut updated_message = message.clone();
        updated_message.status =
            safe_update_message_status(&message.status, MessageStatus::Delivered);
        updated_message.sequence = None;

        // If message has a file, save it to our VFS
        if let Some(ref mut file_info) = updated_message.file_info {
            let is_image = updated_message.message_type == MessageType::Image;
            let original_url = file_info.url.clone();

            let file_data = match file_info.url_kind() {
                crate::types::FileUrlKind::CompressedBase64(rest) => {
                    let compressed_data = match base64_decode(rest) {
                        Ok(data) => data,
                        Err(e) => {
                            log_debug!("Failed to decode compressed file: {}", e);
                            vec![]
                        }
                    };
                    match decompress_data(&compressed_data) {
                        Ok(data) => data,
                        Err(e) => {
                            log_debug!("Failed to decompress file: {}", e);
                            vec![]
                        }
                    }
                }
                crate::types::FileUrlKind::DataUrl(data_url) => {
                    if let Some(comma_pos) = data_url.find(',') {
                        let base64_data = &data_url[comma_pos + 1..];
                        match base64_decode(base64_data) {
                            Ok(data) => data,
                            Err(e) => {
                                log_debug!("Failed to decode file data: {}", e);
                                vec![]
                            }
                        }
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            };

            if !file_data.is_empty() {
                // Save to VFS
                let package_id = our().package_id();
                let file_id = format!("{}_{}", updated_message.timestamp, rand::random::<u32>());
                let vfs_path = format!(
                    "/{}/files/{}/{}",
                    package_id,
                    chat_id.replace(":", "_"),
                    file_id
                );

                // Create directory if it doesn't exist
                let dir_path = format!("/{}/files/{}", package_id, chat_id.replace(":", "_"));
                let _ = vfs::open_dir(&dir_path, true, Some(5));

                // Create and write file
                if let Ok(file) = vfs::create_file(&vfs_path, Some(5)) {
                    let _ = file.write(&file_data);
                    log_debug!(
                        "Saved received file {} to VFS at {}",
                        file_info.filename,
                        vfs_path
                    );

                    // For images, keep the data URL for inline display
                    // For files, update to local VFS path
                    if is_image {
                        // Keep the original data URL for images
                        file_info.url = original_url;
                    } else {
                        // Update the file URL to point to our local VFS path
                        file_info.url = format!("/files/{}/{}", chat_id.replace(":", "_"), file_id);
                    }
                }
            }
        }

        // Deduplicate by message ID so delivery retries don't create copies
        let mut is_duplicate = false;
        let mut should_insert = false;
        let mut stored_message: Option<ChatMessage> = None;
        {
            let chat = self
                .chats
                .get_mut(&chat_id)
                .expect("chat should exist after ensure");
            if let Some(existing) = chat
                .messages
                .iter_mut()
                .find(|m| m.id == updated_message.id)
            {
                is_duplicate = true;
                existing.content = updated_message.content.clone();
                existing.timestamp = updated_message.timestamp;
                existing.reply_to = updated_message.reply_to.clone();
                existing.reactions = updated_message.reactions.clone();
                existing.message_type = updated_message.message_type.clone();
                existing.file_info = updated_message.file_info.clone();
                existing.sender = updated_message.sender.clone();
                existing.status =
                    safe_update_message_status(&existing.status, updated_message.status.clone());
                state_changed = true;
            } else {
                should_insert = true;
            }
            let prev_last_activity = chat.last_activity;
            chat.last_activity = chat.last_activity.max(updated_message.timestamp);
            if chat.last_activity != prev_last_activity {
                state_changed = true;
            }
        }

        if should_insert {
            let mut message_to_store = updated_message.clone();
            self.assign_sequence_to_message(&chat_id, &mut message_to_store);
            if let Some(chat) = self.chats.get_mut(&chat_id) {
                chat.messages.push(message_to_store.clone());
                chat.unread_count += 1;
                state_changed = true;
            }
            stored_message = Some(message_to_store);
        }

        if !is_duplicate {
            let message_for_events = stored_message
                .clone()
                .expect("new messages should be stored before broadcasting");
            let chat_snapshot = self.chats.get(&chat_id).cloned();
            // Send to WebSocket connections if any
            if is_new_chat {
                if let Some(chat_update) = chat_snapshot.clone() {
                    let msg = WsServerMessage::ChatUpdate(chat_update);
                    self.broadcast_ws_message(&msg);
                }
            }

            let msg = WsServerMessage::NewMessage(message_for_events.clone());
            self.broadcast_ws_message(&msg);

            // Send push notification if user has notifications enabled AND no active connections
            let chat_notify_enabled = self
                .chats
                .get(&chat_id)
                .map(|chat| chat.notify)
                .unwrap_or(true);
            let global_notify_enabled = self.settings.notify_chats;
            let active_connection_count = self.active_connections.len();
            log_debug!(
                "[NOTIFY] chat_push_gate chat_id={} chat_notify={} global_notify={} active_connections={}",
                chat_id,
                chat_notify_enabled,
                global_notify_enabled,
                active_connection_count
            );
            if chat_notify_enabled && global_notify_enabled && active_connection_count == 0 {
                let chat_id_for_push = chat_id.clone();
                let message_for_push = message_for_events.clone();
                spawn(async move {
                    send_push_notification_for_message(
                        &message_for_push.sender,
                        &message_for_push.content,
                        &chat_id_for_push,
                    )
                    .await;
                });
            } else {
                log_debug!(
                    "[NOTIFY] chat_push_skip chat_id={} chat_notify={} global_notify={} active_connections={}",
                    chat_id,
                    chat_notify_enabled,
                    global_notify_enabled,
                    active_connection_count
                );
            }
        }

        if state_changed {
            self.rebuild_chat_search(&chat_id);
        }

        // Send acknowledgment back to sender using generated RPC
        let sender = message.sender.clone();
        let msg_id = message.id.clone();

        let target = Address::from((sender.as_str(), OUR_PROCESS_ID));

        // Send acknowledgment using generated RPC method
        let _ = receive_message_ack_remote_rpc(&target, msg_id).await;

        Ok(())
    }
    // Remote handler for receiving reactions
    #[remote]
    async fn receive_reaction(
        &mut self,
        message_id: String,
        emoji: String,
        mut user: String,
    ) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            if user != caller_node {
                log_debug!(
                    "[SEC] receive_reaction rejected spoofed user={} source={}",
                    user,
                    caller_node
                );
                return Err("receive_reaction rejected spoofed user".to_string());
            }
            user = caller_node.clone();
        }
        log_debug!(
            "Received reaction {} from {} for message {}",
            emoji,
            user,
            message_id
        );

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let reaction = MessageReaction {
            emoji: emoji.clone(),
            user: user.clone(),
            timestamp,
        };

        let mut update: Option<WsServerMessage> = None;

        if is_local_call {
            // Local calls are used for tests/debug tooling; keep broad search semantics.
            for chat in self.chats.values_mut() {
                if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                    if !message
                        .reactions
                        .iter()
                        .any(|r| r.user == reaction.user && r.emoji == reaction.emoji)
                    {
                        message.reactions.push(reaction.clone());
                        update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                    }
                    break;
                }
            }
        } else {
            // Remote callers may only mutate chats that involve them.
            let expected_chat_id = Self::normalize_chat_id(&caller_node, &our().node);
            if let Some(chat) = self.chats.get_mut(&expected_chat_id) {
                if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                    if !message
                        .reactions
                        .iter()
                        .any(|r| r.user == reaction.user && r.emoji == reaction.emoji)
                    {
                        message.reactions.push(reaction.clone());
                        update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                    }
                }
            }
        }

        if let Some(chat_update) = update {
            self.broadcast_ws_message(&chat_update);
        }

        // Not an error - might be a reaction for a message we don't have
        Ok(())
    }

    // Remote handler for removing reactions
    #[remote]
    async fn receive_reaction_remove(
        &mut self,
        message_id: String,
        emoji: String,
        mut user: String,
    ) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            if user != caller_node {
                log_debug!(
                    "[SEC] receive_reaction_remove rejected spoofed user={} source={}",
                    user,
                    caller_node
                );
                return Err("receive_reaction_remove rejected spoofed user".to_string());
            }
            user = caller_node.clone();
        }
        log_debug!(
            "Received reaction removal {} from {} for message {}",
            emoji,
            user,
            message_id
        );

        let mut update: Option<WsServerMessage> = None;
        if is_local_call {
            for chat in self.chats.values_mut() {
                if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                    if let Some(pos) = message
                        .reactions
                        .iter()
                        .position(|r| r.user == user && r.emoji == emoji)
                    {
                        message.reactions.remove(pos);
                        update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                    }
                    break;
                }
            }
        } else {
            let expected_chat_id = Self::normalize_chat_id(&caller_node, &our().node);
            if let Some(chat) = self.chats.get_mut(&expected_chat_id) {
                if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                    if let Some(pos) = message
                        .reactions
                        .iter()
                        .position(|r| r.user == user && r.emoji == emoji)
                    {
                        message.reactions.remove(pos);
                        update = Some(WsServerMessage::ChatUpdate(chat.clone()));
                    }
                }
            }
        }

        if let Some(chat_update) = update {
            self.broadcast_ws_message(&chat_update);
        }

        Ok(())
    }

    #[remote]
    async fn receive_message_edit(
        &mut self,
        chat_id: String,
        message_id: String,
        new_content: String,
    ) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            let expected_chat_id = Self::normalize_chat_id(&caller_node, &our().node);
            if chat_id != expected_chat_id {
                log_debug!(
                    "[SEC] receive_message_edit rejected spoofed chat_id={} expected={} source={}",
                    chat_id,
                    expected_chat_id,
                    caller_node
                );
                return Err("receive_message_edit rejected spoofed chat_id".to_string());
            }
        }
        let mut chat_update: Option<WsServerMessage> = None;
        let mut needs_rebuild = false;

        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(message) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                if !is_local_call && message.sender != caller_node {
                    log_debug!(
                        "[SEC] receive_message_edit rejected edit from {} for message sent by {}",
                        caller_node,
                        message.sender
                    );
                    return Err("receive_message_edit rejected unauthorized edit".to_string());
                }
                if message.message_type == MessageType::Payment {
                    return Err("receive_message_edit rejected payment message edit".to_string());
                }
                message.content = new_content;
                needs_rebuild = true;
                chat_update = Some(WsServerMessage::ChatUpdate(chat.clone()));
            }
        }

        if needs_rebuild {
            self.rebuild_chat_search(&chat_id);
        }

        if let Some(update) = chat_update {
            self.broadcast_ws_message(&update);
        } else {
            log_debug!(
                "receive_message_edit: message {} in chat {} not found; dropping edit",
                message_id,
                chat_id
            );
        }

        Ok(())
    }

    // Remote handler for receiving message acknowledgments
    #[remote]
    async fn receive_message_ack(&mut self, message_id: String) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        log_debug!("Received ACK for message {}", message_id);
        // This ACK is from the remote node confirming they received our message
        // We need to find OUR sent message and update its status to Delivered

        let mut update_payload: Option<(String, WsServerMessage)> = None;

        for chat in self.chats.values_mut() {
            if !is_local_call && chat.counterparty != caller_node {
                continue;
            }
            if let Some(message) = chat
                .messages
                .iter_mut()
                .find(|m| m.id == message_id && m.sender == our().node)
            {
                log_debug!("Updating sent message {} status to Delivered", message_id);
                message.status =
                    safe_update_message_status(&message.status, MessageStatus::Delivered);
                update_payload = Some((
                    chat.counterparty.clone(),
                    WsServerMessage::ChatUpdate(chat.clone()),
                ));
                break;
            }
        }

        if let Some((counterparty, chat_update)) = update_payload {
            self.enqueue_delivery_flush(&counterparty);

            // Send ChatUpdate with the delivered status
            for &channel_id in self.ws_connections.keys() {
                log_debug!(
                    "Sending ChatUpdate for delivered message to channel {}",
                    channel_id
                );
                self.push_ws_message(channel_id, &chat_update);
            }
            return Ok(());
        }
        log_debug!("Sent message {} not found for ACK", message_id);
        // Not an error - might be an ACK for a message we don't have anymore
        Ok(())
    }

    #[remote]
    async fn receive_message_deletion(
        &mut self,
        message_id: String,
        chat_id: String,
    ) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            let expected_chat_id = Self::normalize_chat_id(&caller_node, &our().node);
            if chat_id != expected_chat_id {
                log_debug!(
                    "[SEC] receive_message_deletion rejected spoofed chat_id={} expected={} source={}",
                    chat_id, expected_chat_id, caller_node
                );
                return Err("receive_message_deletion rejected spoofed chat_id".to_string());
            }
        }
        log_debug!(
            "Received deletion request for message {} in chat {}",
            message_id,
            chat_id
        );

        let mut chat_update: Option<WsServerMessage> = None;
        let mut needs_rebuild = false;

        if let Some(chat) = self.chats.get_mut(&chat_id) {
            if let Some(pos) = chat.messages.iter().position(|m| m.id == message_id) {
                if !is_local_call && chat.messages[pos].sender != caller_node {
                    log_debug!(
                        "[SEC] receive_message_deletion rejected delete from {} for message sent by {}",
                        caller_node, chat.messages[pos].sender
                    );
                    return Err("receive_message_deletion rejected unauthorized delete".to_string());
                }
                if chat.messages[pos].message_type == MessageType::Payment {
                    return Err(
                        "receive_message_deletion rejected payment message delete".to_string()
                    );
                }
                chat.messages.remove(pos);
                needs_rebuild = true;
                log_debug!("Deleted message {} from chat {}", message_id, chat_id);
                chat_update = Some(WsServerMessage::ChatUpdate(chat.clone()));
            }
        }

        if needs_rebuild {
            self.rebuild_chat_search(&chat_id);
        }

        if let Some(update) = chat_update {
            self.broadcast_ws_message(&update);
        }

        Ok(())
    }

    #[remote]
    async fn receive_profile_update(
        &mut self,
        mut node: String,
        profile: UserProfile,
    ) -> Result<(), String> {
        let caller_node = source().node.clone();
        let is_local_call = caller_node == our().node;
        if !is_local_call {
            if node != caller_node {
                log_debug!(
                    "[SEC] receive_profile_update rejected spoofed node={} source={}",
                    node,
                    caller_node
                );
                return Err("receive_profile_update rejected spoofed node".to_string());
            }
            node = caller_node;
        }
        log_debug!("Received profile update from {}: {:?}", node, profile);

        // Store the profile
        self.node_profiles.insert(node.clone(), profile.clone());
        if node != our().node {
            self.reconcile_dm_chats_for_counterparty(&node);
            Self::sync_contact_profile_fields(node.clone(), profile.clone());
        }

        // Update all chats with this counterparty
        let mut updates = Vec::new();
        let mut updated_chat_ids = Vec::new();
        for chat in self.chats.values_mut() {
            if chat.counterparty == node {
                chat.counterparty_profile = Some(profile.clone());
                updated_chat_ids.push(chat.id.clone());
                updates.push(WsServerMessage::ChatUpdate(chat.clone()));
            }
        }
        for chat_id in updated_chat_ids {
            self.rebuild_chat_search(&chat_id);
        }
        for update in updates {
            self.broadcast_ws_message(&update);
        }

        Ok(())
    }

    // PUBLIC BROWSER CHAT ENDPOINTS

    #[http(path = "/public")]
    async fn serve_public_chat(&self) -> Result<String, String> {
        // Serve the browser chat HTML
        Ok(include_str!("../../ui/public/browser-chat.html").to_string())
    }

    #[http(path = "/public/join-*")]
    async fn serve_join_link(&self) -> Result<String, String> {
        // Serve the browser chat HTML for join links
        Ok(include_str!("../../ui/public/browser-chat.html").to_string())
    }

    #[http(method = "GET", path = "/files/*")]
    async fn serve_file(&self) -> Result<Vec<u8>, String> {
        let path = match get_path() {
            Some(path) => path,
            None => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                return Err("Invalid file path".to_string());
            }
        };

        let rest = match path.strip_prefix("/files/") {
            Some(rest) => rest,
            None => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                return Err("Invalid file path".to_string());
            }
        };

        let mut segments = rest.split('/');
        let chat_id = match segments.next() {
            Some(chat_id) if !chat_id.is_empty() => chat_id,
            _ => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                return Err("Invalid file path".to_string());
            }
        };
        let file_id = match segments.next() {
            Some(file_id) if !file_id.is_empty() => file_id,
            _ => {
                set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
                return Err("Invalid file path".to_string());
            }
        };

        // Build VFS path
        let package_id = our().package_id();
        let vfs_path = format!("/{}/files/{}/{}", package_id, chat_id, file_id);

        // Read file from VFS
        let file = vfs::open_file(&vfs_path, false, Some(5)).map_err(|e| {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            format!("Failed to open file: {:?}", e)
        })?;

        let file_data = file.read().map_err(|e| {
            set_response_status(hyperware_process_lib::http::StatusCode::NOT_FOUND);
            format!("Failed to read file: {:?}", e)
        })?;

        Ok(file_data)
    }
    // SEARCH

    // uncomment #[remote] for tests
    // #[remote]
    #[http]
    async fn search_chats(&self, req: SearchChatsReq) -> Result<Vec<Chat>, String> {
        let query = req.query.to_lowercase();
        let results: Vec<Chat> = self
            .chats
            .values()
            .filter(|chat| {
                chat.counterparty.to_lowercase().contains(&query)
                    || chat
                        .counterparty_profile
                        .as_ref()
                        .map(|profile| profile.name.to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || chat
                        .messages
                        .iter()
                        .any(|m| m.content.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();

        Ok(results)
    }

    #[http]
    async fn search_index(&self, req: SearchIndexReq) -> Result<SearchIndexRes, String> {
        let results = self.search_index.search(&req.query, req.scope, req.limit);
        Ok(SearchIndexRes { results })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn crdt_group_state_vector(
        &mut self,
        req: CrdtGroupStateVectorReq,
    ) -> Result<CrdtStateVectorRes, String> {
        if self.group_needs_bootstrap(&req.group_id) {
            return Err(format!(
                "Group {} is pending bootstrap and cannot serve CRDT requests",
                req.group_id
            ));
        }

        self.require_hub_access(&req.group_id, &our().node)
            .map_err(|err| format!("hub access denied: {}", err))?;

        let manager = self
            .ensure_group_doc_manager(&req.group_id)
            .map_err(|e| format!("Failed to init group CRDT: {:?}", e))?;

        let (doc_id, state_vector) = {
            let doc = manager.doc();
            (doc.id().to_string(), doc.state_vector())
        };
        log_crdt_event(&doc_id, "crdt_group_state_vector", &state_vector, None);
        let encoded = base64_encode(&state_vector.encode_v1());
        manager.set_last_state_vector(state_vector);

        Ok(CrdtStateVectorRes {
            state_vector: encoded,
        })
    }

    #[remote]
    #[local]
    #[http]
    async fn crdt_group_update(
        &mut self,
        req: CrdtGroupUpdateReq,
    ) -> Result<CrdtUpdateRes, String> {
        if self.group_needs_bootstrap(&req.group_id) {
            return Err(format!(
                "Group {} is pending bootstrap and cannot serve CRDT requests",
                req.group_id
            ));
        }

        // V2.2: Validate sender is an active member for remote requests
        // Only check if the group exists - if group doesn't exist, let it fail naturally later
        let sender_addr = source();
        let sender_node = sender_addr.node.clone();
        if sender_node != our().node && self.groups.contains_key(&req.group_id) {
            // Remote request - validate sender is an active member
            self.require_subscriber_access(&req.group_id, &sender_node)
                .map_err(|e| {
                    format!(
                        "CRDT update denied: sender {} not authorized: {}",
                        sender_node, e
                    )
                })?;
        }

        self.require_hub_access(&req.group_id, &our().node)
            .map_err(|err| format!("hub access denied: {}", err))?;

        let manager = self
            .ensure_group_doc_manager(&req.group_id)
            .map_err(|e| format!("Failed to init group CRDT: {:?}", e))?;
        log_debug!(
            "[CRDT][{}] crdt_group_update: state_vector={:?} doc_id={} manager_ptr={:p}",
            req.group_id,
            req.state_vector,
            manager.doc().id(),
            manager.doc()
        );

        if let Ok(state) = manager.doc().read_state() {
            log_group_state_summary(manager.doc().id(), "crdt_group_update:sender_state", &state);
            log_debug!(
                "[CRDT][{}] sender_state members={:?}",
                manager.doc().id(),
                state
                    .group
                    .members
                    .iter()
                    .map(|(k, v)| (k, (&v.role_id, v.status)))
                    .collect::<Vec<_>>()
            );
        }

        let state_vector =
            if let Some(encoded_sv) = req.state_vector.as_ref().filter(|s| !s.trim().is_empty()) {
                let trimmed = encoded_sv.trim();
                let bytes = base64_decode(trimmed)
                    .map_err(|e| format!("Invalid state vector payload: {e}"))?;
                Some(
                    StateVector::decode_v1(&bytes)
                        .map_err(|e| format!("Invalid state vector bytes: {:?}", e))?,
                )
            } else {
                None
            };

        let (doc_id, doc_vector, update_bytes) = {
            let doc = manager.doc();
            (
                doc.id().to_string(),
                doc.state_vector(),
                doc.encode_update_since(state_vector.as_ref()),
            )
        };
        log_crdt_event(
            &doc_id,
            "crdt_group_update",
            &doc_vector,
            Some(update_bytes.len()),
        );

        let update_payload = base64_encode(&update_bytes);
        self.publish_group_delta(&req.group_id, &update_payload);

        Ok(CrdtUpdateRes {
            doc_id,
            update_payload,
        })
    }

    #[remote]
    #[local]
    #[http]
    async fn crdt_group_apply_update(
        &mut self,
        req: CrdtGroupApplyReq,
    ) -> Result<CrdtApplyRes, String> {
        let group_id = req.group_id.clone();

        // V2.2: Validate sender is an active member for remote requests
        // Only check if the group exists - if group doesn't exist, let it fail naturally later
        let sender_addr = source();
        let sender_node = sender_addr.node.clone();
        if sender_node != our().node && self.groups.contains_key(&group_id) {
            // Remote request - validate sender is an active member
            self.require_subscriber_access(&group_id, &sender_node)
                .map_err(|e| {
                    format!(
                        "CRDT apply denied: sender {} not authorized: {}",
                        sender_node, e
                    )
                })?;
        }

        self.apply_group_update_payload(
            &group_id,
            &req.update_payload,
            "crdt_group_apply_update",
            req.acl_version,
            false,
        )?;
        Ok(CrdtApplyRes { applied: true })
    }

    #[remote]
    #[local]
    #[http]
    async fn crdt_group_snapshot(
        &mut self,
        req: CrdtGroupSnapshotReq,
    ) -> Result<CrdtUpdateRes, String> {
        if self.group_needs_bootstrap(&req.group_id) {
            return Err(format!(
                "Group {} is pending bootstrap and cannot serve CRDT requests",
                req.group_id
            ));
        }

        // V2.2: Validate sender is an active member for remote requests
        // Only check if the group exists - if group doesn't exist, let it fail naturally later
        let sender_addr = source();
        let sender_node = sender_addr.node.clone();
        if sender_node != our().node && self.groups.contains_key(&req.group_id) {
            // Remote request - validate sender is an active member
            self.require_subscriber_access(&req.group_id, &sender_node)
                .map_err(|e| {
                    format!(
                        "CRDT snapshot denied: sender {} not authorized: {}",
                        sender_node, e
                    )
                })?;
        }

        self.require_hub_access(&req.group_id, &our().node)
            .map_err(|err| format!("hub access denied: {}", err))?;

        let manager = self
            .ensure_group_doc_manager(&req.group_id)
            .map_err(|e| format!("Failed to init group CRDT: {:?}", e))?;

        if let Ok(state) = manager.doc().read_state() {
            log_group_state_summary(
                manager.doc().id(),
                "crdt_group_snapshot:sender_state",
                &state,
            );
        }

        let (doc_id, state_vector, update_bytes) = {
            let doc = manager.doc();
            (
                doc.id().to_string(),
                doc.state_vector(),
                doc.encode_update_since(None),
            )
        };

        log_crdt_event(
            &doc_id,
            "crdt_group_snapshot",
            &state_vector,
            Some(update_bytes.len()),
        );

        let update_payload = base64_encode(&update_bytes);
        Ok(CrdtUpdateRes {
            doc_id,
            update_payload,
        })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn replication_work(&mut self) -> Result<(), String> {
        self.run_replication_work_guarded().await
    }

    /// Immediately push a snapshot to a peer (bypassing the debounced queue).
    /// This is called when a member is invited to get them bootstrapped immediately.
    #[local]
    #[http]
    async fn push_snapshot_to_peer(&mut self, req: PushSnapshotToPeerReq) -> Result<(), String> {
        log_debug!(
            "[REPL][{}] push_snapshot_to_peer invoked peer={}",
            req.group_id,
            req.peer
        );
        let task = ReplicationTask {
            group_id: req.group_id,
            peer: req.peer,
            kind: ReplicationKind::PushSnapshot,
            since: None,
            attempt: 0,
            not_before: ChatState::now_secs(),
        };
        self.process_replication_task(task).await;
        Ok(())
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn admin_replication_state(
        &mut self,
        req: AdminReplicationStateReq,
    ) -> Result<AdminReplicationStateRes, String> {
        self.refresh_bootstrap_flags();
        log_debug!(
            "[ADMIN] admin_replication_state invoked filter={:?} group_count={}",
            req.group_id,
            self.groups.len()
        );
        let filter = req.group_id;
        let now = ChatState::now_secs();
        let groups: Vec<GroupReplicationState> = self
            .groups
            .iter()
            .filter(|(id, _)| filter.as_ref().map_or(true, |gid| gid == *id))
            .map(|(group_id, group)| {
                let local_member_status = group.members.get(&our().node).map(|m| m.status);
                log_debug!(
                    "[ADMIN][{}] pending_bootstrap={} local_member_status={:?} whitelist_version={:?} hub_topic={} sub_topic={}",
                    group_id,
                    self.group_needs_bootstrap(group_id),
                    local_member_status,
                    self.pubsub.whitelist(group_id).map(|w| w.version()),
                    group.routing.hub_topic,
                    group.routing.subscriber_topic,
                );
                let sub_lag = group
                    .delivery
                    .subscriber_cursors
                    .get(&our().node)
                    .map(|c| now.saturating_sub(c.updated_at));
                let hub_lag = group
                    .delivery
                    .hub_cursors
                    .get(&our().node)
                    .map(|c| now.saturating_sub(c.updated_at));
                GroupReplicationState {
                    group_id: group_id.clone(),
                    pending_bootstrap: self.group_needs_bootstrap(group_id),
                    routing: group.routing.clone(),
                    hubs: group.hubs.active.iter().cloned().collect(),
                    subscribers: group.subscribers.entries.keys().cloned().collect(),
                    hub_cursors: group.delivery.hub_cursors.clone(),
                    subscriber_cursors: group.delivery.subscriber_cursors.clone(),
                    whitelist_version: self.pubsub.whitelist(group_id).map(|w| w.version()),
                    subscriber_lag_secs: sub_lag,
                    hub_lag_secs: hub_lag,
                }
            })
            .collect();

        Ok(AdminReplicationStateRes {
            metrics: self.replication_metrics.clone(),
            groups,
        })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn admin_whitelist(&self, req: AdminWhitelistReq) -> Result<AdminWhitelistRes, String> {
        log_debug!(
            "[ADMIN] admin_whitelist invoked group_id={} has_whitelist={}",
            req.group_id,
            self.pubsub.whitelist(&req.group_id).is_some()
        );
        let whitelist = self
            .pubsub
            .whitelist(&req.group_id)
            .ok_or_else(|| "whitelist missing".to_string())?;

        let fmt_pattern = |pattern: &hyperware_pubsub_core::whitelist::TopicPattern| match pattern {
            hyperware_pubsub_core::whitelist::TopicPattern::Exact(p) => {
                format!("exact:{p}")
            }
            hyperware_pubsub_core::whitelist::TopicPattern::Prefix(p) => {
                format!("prefix:{p}")
            }
        };

        let entries = whitelist
            .entries()
            .iter()
            .map(|(node, access)| {
                let expires_at = access
                    .expires_at
                    .and_then(|ts| ts.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                WhitelistEntryDebug {
                    node: node.0.clone(),
                    publish: access.publish.iter().map(fmt_pattern).collect(),
                    subscribe: access.subscribe.iter().map(fmt_pattern).collect(),
                    audiences: access.audiences.iter().cloned().collect(),
                    features: access.features.iter().cloned().collect(),
                    expires_at,
                }
            })
            .collect();

        Ok(AdminWhitelistRes {
            group_id: req.group_id,
            version: whitelist.version(),
            entries,
        })
    }

    // uncomment #[remote] for tests
    // #[remote]
    #[local]
    #[http]
    async fn admin_subscriber_events(
        &mut self,
        req: SubscriberEventsReq,
    ) -> Result<SubscriberEventsRes, String> {
        log_debug!(
            "[ADMIN] admin_subscriber_events invoked clear={} take={:?} buffered={}",
            req.clear,
            req.take,
            self.subscriber_events.len()
        );
        let take = req.take.unwrap_or(50);
        let events = if req.clear {
            // Clearing should drop pending events and return an empty list to signal nothing remains.
            self.subscriber_events.clear();
            Vec::new()
        } else {
            let len = self.subscriber_events.len();
            let start = len.saturating_sub(take);
            self.subscriber_events.iter().skip(start).cloned().collect()
        };
        Ok(SubscriberEventsRes { events })
    }

    // SPIDER INTEGRATION

    #[http]
    async fn spider_connect(
        &mut self,
        force_new: Option<bool>,
    ) -> Result<SpiderConnectResult, String> {
        const SPIDER_PROCESS_ID: (&str, &str, &str) = ("spider", "spider", "sys");

        let should_force = force_new.unwrap_or(false);
        log_debug!(
            "[SPIDER] spider_connect called, force_new={:?}, should_force={}",
            force_new,
            should_force
        );
        log_debug!(
            "[SPIDER] cached key exists: {}",
            self.spider_api_key.is_some()
        );

        if !should_force {
            if let Some(existing) = self.spider_api_key.clone() {
                log_debug!(
                    "[SPIDER] Validating cached key: {}...",
                    &existing[..8.min(existing.len())]
                );
                // Validate the cached key before returning it
                if self.validate_spider_key(&existing).await {
                    log_debug!("[SPIDER] Cached key is valid, returning it");
                    return Ok(SpiderConnectResult { api_key: existing });
                }
                log_debug!("[SPIDER] cached spider API key is invalid, creating new one");
            }
        }

        // Always use a unique name to ensure Spider creates a fresh key
        let key_name = format!(
            "homepage-{}-{}",
            our().node.clone(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        log_debug!("[SPIDER] Creating new key with name: {}", key_name);

        let body = serde_json::json!({
            "CreateSpiderKey": {
                "name": key_name,
                "permissions": vec!["read", "write", "chat"],
                "adminKey": "",
            }
        });
        log_debug!("[SPIDER] Sending CreateSpiderKey request to spider:spider:sys");
        let request = ProcessRequest::to(Address::new("our", SPIDER_PROCESS_ID))
            .body(
                serde_json::to_vec(&body)
                    .map_err(|err| format!("failed to serialize spider key request: {err}"))?,
            )
            .expects_response(5);

        let parsed: Result<SpiderApiKey, String> =
            hyperapp::send(request).await.map_err(|err| {
                log_debug!("[SPIDER] Failed to contact spider: {}", err);
                format!("failed to contact spider: {err}")
            })?;

        match parsed {
            Ok(key) => {
                log_debug!(
                    "[SPIDER] Successfully created key: {}...",
                    &key.key[..8.min(key.key.len())]
                );
                self.spider_api_key = Some(key.key.clone());
                Ok(SpiderConnectResult { api_key: key.key })
            }
            Err(err) => {
                log_debug!("[SPIDER] Spider refused to create key: {}", err);
                Err(format!("spider refused to create key: {err}"))
            }
        }
    }

    #[http]
    async fn spider_status(&self) -> Result<SpiderStatusInfo, String> {
        const SPIDER_PROCESS_ID: (&str, &str, &str) = ("spider", "spider", "sys");

        log_debug!("[SPIDER] spider_status called");
        let ping_body = serde_json::json!({ "Ping": null });
        let request = ProcessRequest::to(Address::new("our", SPIDER_PROCESS_ID))
            .body(
                serde_json::to_vec(&ping_body)
                    .map_err(|err| format!("failed to serialize ping: {err}"))?,
            )
            .expects_response(2);
        let ping_result = hyperapp::send::<serde_json::Value>(request).await;
        let available = ping_result.is_ok();
        log_debug!(
            "[SPIDER] Ping result: {:?}, available: {}",
            ping_result,
            available
        );

        let status = SpiderStatusInfo {
            connected: self.spider_api_key.is_some() && available,
            has_api_key: self.spider_api_key.is_some(),
            spider_available: available,
        };
        log_debug!(
            "[SPIDER] Returning status: connected={}, has_api_key={}, spider_available={}",
            status.connected,
            status.has_api_key,
            status.spider_available
        );
        Ok(status)
    }

    #[http]
    async fn spider_get_history(&self) -> Result<SpiderHistory, String> {
        Ok(SpiderHistory {
            messages: self.spider_history.clone(),
        })
    }

    #[http]
    async fn spider_set_history(&mut self, request: SpiderSetHistoryReq) -> Result<(), String> {
        self.spider_history = request.messages;
        Ok(())
    }

    // WEBSOCKET HANDLERS

    #[ws]
    fn websocket(&mut self, channel_id: u32, message_type: WsMessageType, blob: LazyLoadBlob) {
        // We'll differentiate between public and private connections via authentication
        match message_type {
            WsMessageType::Close => {
                log_debug!("[WS_DEBUG] WebSocket Close received for channel {}, ws_connections before: {:?}", channel_id, self.ws_connections.keys().collect::<Vec<_>>());
                // Clean up connection
                if let Some(node) = self.ws_connections.remove(&channel_id) {
                    // Broadcast status update
                    let status_msg = WsServerMessage::StatusUpdate {
                        node: node.clone(),
                        status: "offline".to_string(),
                    };
                    self.broadcast_ws_message(&status_msg);
                }

                // Clean up browser connections
                self.browser_connections.retain(|_, &mut v| v != channel_id);
                self.active_connections.remove(&channel_id);
            }
            WsMessageType::Text => {
                // Parse and handle client message
                if let Ok(payload) = String::from_utf8(blob.bytes.clone()) {
                    match serde_json::from_str::<WsClientMessage>(&payload) {
                        Ok(msg) => {
                            log_debug!(
                                "WebSocket: Received message from channel {}: {:?}",
                                channel_id,
                                msg
                            );
                            // Initialize connection if not already present
                            if !self.ws_connections.contains_key(&channel_id)
                                && !self
                                    .browser_connections
                                    .values()
                                    .any(|&ch| ch == channel_id)
                            {
                                log_debug!(
                                    "[WS_DEBUG] New connection from channel {}, ws_connections before: {:?}",
                                    channel_id, self.ws_connections.keys().collect::<Vec<_>>()
                                );
                                self.ws_connections.insert(channel_id, our().node.clone());
                                log_debug!(
                                    "[WS_DEBUG] After insert, ws_connections: {:?}",
                                    self.ws_connections.keys().collect::<Vec<_>>()
                                );

                                // Send all existing chats to the new connection
                                log_debug!(
                                    "WebSocket: Sending {} chats to new connection",
                                    self.chats.len()
                                );
                                for chat in self.chats.values() {
                                    log_debug!(
                                        "WebSocket: Sending chat {} with {} messages",
                                        chat.id,
                                        chat.messages.len()
                                    );
                                    let chat_update = WsServerMessage::ChatUpdate(chat.clone());
                                    self.push_ws_message(channel_id, &chat_update);
                                }
                                log_debug!(
                                    "WebSocket: Initial chat sync complete for channel {}",
                                    channel_id
                                );
                            }

                            // Check if this is a browser chat authentication
                            if let WsClientMessage::AuthWithKey { .. } = &msg {
                                self.handle_browser_message(channel_id, msg);
                            } else if self
                                .browser_connections
                                .values()
                                .any(|&ch| ch == channel_id)
                            {
                                // If already authenticated as browser
                                self.handle_browser_message(channel_id, msg);
                            } else {
                                // Node-to-node message
                                self.handle_client_message(channel_id, msg);
                            }
                        }
                        Err(e) => {
                            let error = WsServerMessage::Error {
                                message: format!("Invalid message format: {}", e),
                            };
                            self.push_ws_message(channel_id, &error);
                        }
                    }
                }
            }
            WsMessageType::Binary => {
                // Handle binary messages if needed (e.g., for voice calls later)
                log_debug!("Binary message received on channel {}", channel_id);
            }
            WsMessageType::Ping | WsMessageType::Pong => {
                // Ignore ping/pong messages
            }
        }
    }
}

// Helper methods (outside hyperapp impl)
impl ChatState {
    // GROUP OPERATIONS
    fn join_group_link_internal(
        &mut self,
        key: String,
        candidate: NodeId,
    ) -> Result<JoinGroupLinkRes, String> {
        let join_key = self
            .group_join_keys
            .get(&key)
            .ok_or_else(|| "Join link not found".to_string())?;
        if join_key.is_revoked {
            return Err("Join link revoked".to_string());
        }

        let group_id = join_key.group_id.clone();
        self.join_public_group(&group_id, candidate)
            .map_err(|err| err.to_string())?;
        Ok(JoinGroupLinkRes { group_id })
    }

    // MESSAGE OPERATIONS

    /// MessageStatus lifecycle:
    /// - New outbound messages start as `Sending`, are marked `Sent` once staged locally and
    ///   broadcast to connected clients, and move to `Delivered` when an ack arrives.
    /// - Counterparty delivery uses RPC with an offline queue; WebSocket delivery is used if the
    ///   counterparty is connected locally.
    /// - Failures in RPC enqueue a retry via the delivery worker; persistent failure can be marked
    ///   as `Failed` by the delivery pipeline.
    /// - Frontends may optimistically render temp IDs; the `MessageAck` emitted to the origin
    ///   channel contains the canonical message_id for dedupe/update.
    fn stage_outgoing_message(
        &mut self,
        chat_id: &str,
        mut message: ChatMessage,
        origin_channel: Option<u32>,
    ) -> (String, ChatMessage) {
        self.assign_sequence_to_message(chat_id, &mut message);

        let chat_snapshot = {
            let chat = self.get_or_create_chat(chat_id, message.timestamp, None, None);
            chat.messages.push(message.clone());
            chat.last_activity = message.timestamp;

            if let Some(msg) = chat.messages.iter_mut().find(|m| m.id == message.id) {
                msg.status = safe_update_message_status(&msg.status, MessageStatus::Sent);
            }

            chat.clone()
        };
        self.rebuild_chat_search(chat_id);

        let counterparty = chat_snapshot.counterparty.clone();
        let stored_message = chat_snapshot
            .messages
            .iter()
            .find(|m| m.id == message.id)
            .cloned()
            .unwrap_or(message);

        self.broadcast_ws_message(&WsServerMessage::ChatUpdate(chat_snapshot));

        if let Some(ch_id) = origin_channel {
            self.push_ws_message(
                ch_id,
                &WsServerMessage::MessageAck {
                    message_id: stored_message.id.clone(),
                },
            );
        }

        (counterparty, stored_message)
    }

    fn dispatch_outgoing_message(&self, counterparty: String, message: ChatMessage) {
        // Try fast-path WebSocket delivery if the counterparty is connected locally; otherwise
        // fall back to RPC with offline queue retry.
        if let Some((&ch_id, _)) = self
            .ws_connections
            .iter()
            .find(|(_, node)| *node == &counterparty)
        {
            self.push_ws_message(ch_id, &WsServerMessage::NewMessage(message));
        } else {
            ChatState::spawn_delivery_attempt(
                counterparty,
                message,
                self.delivery_tx.clone(),
                self.pending_deliveries.clone(),
            );
        }
    }

    fn send_message_internal(
        &mut self,
        chat_id: &str,
        content: String,
        reply_to: Option<String>,
        origin_channel: Option<u32>,
    ) -> Result<ChatMessage, String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message_id = format!("{}:{}", timestamp, rand::random::<u32>());

        let message = ChatMessage {
            id: message_id.clone(),
            sender: our().node.clone(),
            content,
            timestamp,
            sequence: None,
            status: MessageStatus::Sending,
            reply_to,
            reactions: Vec::new(),
            message_type: MessageType::Text,
            file_info: None,
            payment_info: None,
        };

        let (counterparty, stored_message) =
            self.stage_outgoing_message(chat_id, message, origin_channel);
        self.dispatch_outgoing_message(counterparty, stored_message.clone());

        // Return the latest stored version (with sequence/status) if available.
        Ok(self
            .chats
            .get(chat_id)
            .and_then(|chat| {
                chat.messages
                    .iter()
                    .find(|m| m.id == stored_message.id)
                    .cloned()
            })
            .unwrap_or(stored_message))
    }

    fn spawn_delivery_attempt(
        counterparty: String,
        msg_to_send: ChatMessage,
        delivery_tx: DeliveryTx,
        pending_deliveries: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    ) {
        let message_id_clone = msg_to_send.id.clone();
        spawn(async move {
            let target = Address::from((counterparty.as_str(), OUR_PROCESS_ID));
            let msg_json = serde_json::to_value(&msg_to_send).unwrap();
            let msg_for_rpc: CUChatMessage = serde_json::from_value(msg_json).unwrap();
            match receive_message_remote_rpc(&target, msg_for_rpc).await {
                Ok(_) => {
                    log_debug!(
                        "Message {} sent successfully to {}",
                        message_id_clone,
                        counterparty
                    );
                    // Counterparty will send ACK on success.
                }
                Err(_) => {
                    log_debug!(
                        "Failed to send message {} to {}, adding to delivery queue",
                        message_id_clone,
                        counterparty
                    );
                    ChatState::enqueue_delivery_message_inner(
                        &delivery_tx,
                        &pending_deliveries,
                        &counterparty,
                        msg_to_send,
                    );
                }
            }
        });
    }

    // REPLICATION HELPERS

    async fn run_replication_work_guarded(&mut self) -> Result<(), String> {
        if self
            .replication_work_inflight
            .compare_exchange(false, true, AtomicOrdering::SeqCst, AtomicOrdering::SeqCst)
            .is_err()
        {
            log_debug!("[REPL] replication_work already running, skipping wake");
            return Ok(());
        }
        let res = self.replication_work_inner().await;
        self.replication_work_inflight
            .store(false, AtomicOrdering::SeqCst);
        // If work was enqueued while we were inflight, the wake may have been
        // dropped by the guard above. Re-wake only when there is ready work to
        // avoid tight loops on backoff/future tasks.
        let now = ChatState::now_secs();
        if self
            .replication_queue
            .iter()
            .any(|task| task.not_before <= now)
        {
            log_debug!("[REPL] ready replication tasks remain, re-waking worker");
            self.wake_replication_worker();
        }
        res
    }

    async fn replication_work_inner(&mut self) -> Result<(), String> {
        self.refresh_bootstrap_flags();
        let started = Instant::now();
        let time_budget = Duration::from_secs(12);
        log_debug!(
            "[REPL] replication_work invoked pending_bootstrap={:?} queue_len={} now={}",
            self.groups_pending_bootstrap,
            self.replication_queue.len(),
            ChatState::now_secs()
        );
        let now = ChatState::now_secs();
        let applied = self.consume_broker_topics(32);
        if applied > 0 {
            log_debug!("[REPL] applied {} broker messages", applied);
        }
        self.enqueue_bootstrap_pulls(now);
        self.enqueue_stale_subscriber_replays(now);

        let mut processed = 0usize;
        while processed < 6 {
            if started.elapsed() >= time_budget {
                log_debug!(
                    "[REPL] replication_work time budget exhausted after {} tasks (elapsed {}ms)",
                    processed,
                    started.elapsed().as_millis()
                );
                break;
            }
            let Some(task) = self.next_ready_replication_task(now) else {
                break;
            };
            self.process_replication_task(task).await;
            processed += 1;
        }

        log_debug!(
            "[REPL] replication processed {} (elapsed {}ms) pending_bootstrap={:?}",
            processed,
            started.elapsed().as_millis(),
            self.groups_pending_bootstrap
        );
        if started.elapsed() > Duration::from_secs(5) {
            log_debug!(
                "[REPL_DIAG] replication_work slow_call elapsed_ms={} queue_len_end={} pending_bootstrap_end={:?}",
                started.elapsed().as_millis(),
                self.replication_queue.len(),
                self.groups_pending_bootstrap
            );
        } else {
            log_debug!(
                "[REPL_DIAG] replication_work done elapsed_ms={} queue_len_end={} pending_bootstrap_end={:?}",
                started.elapsed().as_millis(),
                self.replication_queue.len(),
                self.groups_pending_bootstrap
            );
        }
        Ok(())
    }

    async fn process_replication_task(&mut self, task: ReplicationTask) {
        let now = ChatState::now_secs();
        match task.kind {
            ReplicationKind::PushDelta | ReplicationKind::PushSnapshot => {
                let is_hub = self
                    .groups
                    .get(&task.group_id)
                    .map(|g| g.hubs.active.contains(&task.peer))
                    .unwrap_or(false);

                // Check if we're a removed member trying to push our final update.
                // If so, skip the local ACL check - the remote will decide whether to accept.
                let our_member_status = self
                    .groups
                    .get(&task.group_id)
                    .and_then(|g| g.members.get(&our().node))
                    .map(|m| m.status);
                let is_self_removed = our_member_status == Some(MembershipStatus::Removed);

                if !is_self_removed {
                    if is_hub {
                        if let Err(err) = self.require_hub_access(&task.group_id, &our().node) {
                            log_debug!(
                                "[REPL][{}] skip push to {} (local hub publish denied): {}",
                                task.group_id,
                                task.peer,
                                err
                            );
                            self.replication_metrics.acl_skips =
                                self.replication_metrics.acl_skips.saturating_add(1);
                            return;
                        }
                    } else if let Err(err) =
                        self.require_subscriber_access(&task.group_id, &our().node)
                    {
                        log_debug!(
                            "[REPL][{}] skip push to subscriber {} (local publish denied): {}",
                            task.group_id,
                            task.peer,
                            err
                        );
                        self.replication_metrics.acl_skips =
                            self.replication_metrics.acl_skips.saturating_add(1);
                        return;
                    }
                }

                let sv_hint = task
                    .since
                    .as_ref()
                    .and_then(|b| StateVector::decode_v1(b).ok())
                    .or_else(|| self.peer_state_vector(&task.group_id, &task.peer));
                let acl_version = self.pubsub.whitelist(&task.group_id).map(|w| w.version());

                let manager = match self.ensure_group_doc_manager(&task.group_id) {
                    Ok(m) => m,
                    Err(err) => {
                        log_debug!(
                            "[REPL][{}] cannot load doc for {}: {:?}",
                            task.group_id,
                            task.peer,
                            err
                        );
                        self.schedule_backoff(task, now);
                        return;
                    }
                };
                let doc = manager.doc();
                let update_bytes = if let ReplicationKind::PushSnapshot = task.kind {
                    doc.encode_update_since(None)
                } else {
                    doc.encode_update_since(sv_hint.as_ref())
                };
                let state_vector = doc.state_vector();
                if update_bytes.is_empty() {
                    self.update_peer_state_vector(&task.group_id, &task.peer, &state_vector);
                    self.update_delivery_cursor(
                        &task.group_id,
                        &task.peer,
                        is_hub,
                        if is_hub {
                            self.groups
                                .get(&task.group_id)
                                .map(|g| g.routing.hub_topic.clone())
                                .unwrap_or_default()
                        } else {
                            self.groups
                                .get(&task.group_id)
                                .map(|g| g.routing.subscriber_topic.clone())
                                .unwrap_or_default()
                        },
                        None,
                    );
                    return;
                }

                let payload = base64_encode(&update_bytes);
                let body = serde_json::to_vec(&serde_json::json!({
                    "CrdtGroupApplyUpdate": {
                        "group_id": task.group_id,
                        "update_payload": payload,
                        "acl_version": acl_version,
                    }
                }))
                .unwrap_or_default();

                let target = Address::from((task.peer.as_str(), OUR_PROCESS_ID));
                let req = Request::new()
                    .target(target.clone())
                    .body(body)
                    .expects_response(REPL_RPC_TIMEOUT_SECS);
                log_debug!(
                    "[REPL][{}] push kind={:?} peer={} target={:?}",
                    task.group_id,
                    task.kind,
                    task.peer,
                    target
                );
                let rpc_started = Instant::now();
                match send::<serde_json::Value>(req).await {
                    Ok(val) => {
                        log_debug!(
                            "[REPL_DIAG][{}] push roundtrip_ms={} kind={:?} peer={}",
                            task.group_id,
                            rpc_started.elapsed().as_millis(),
                            task.kind,
                            task.peer
                        );
                        let apply_res = val
                            .get("Ok")
                            .cloned()
                            .or_else(|| Some(val.clone()))
                            .and_then(|v| serde_json::from_value::<CrdtApplyRes>(v).ok());
                        if let Some(res) = apply_res {
                            if res.applied {
                                self.update_peer_state_vector(
                                    &task.group_id,
                                    &task.peer,
                                    &state_vector,
                                );
                                let queue_id = if is_hub {
                                    self.groups
                                        .get(&task.group_id)
                                        .map(|g| g.routing.hub_topic.clone())
                                        .unwrap_or_default()
                                } else {
                                    self.groups
                                        .get(&task.group_id)
                                        .map(|g| g.routing.subscriber_topic.clone())
                                        .unwrap_or_default()
                                };
                                self.update_delivery_cursor(
                                    &task.group_id,
                                    &task.peer,
                                    is_hub,
                                    queue_id,
                                    None,
                                );
                            } else {
                                self.schedule_backoff(task, now);
                            }
                        } else {
                            log_debug!(
                                "[REPL][{}] failed to decode apply response from {}: {:?}",
                                task.group_id,
                                task.peer,
                                val
                            );
                            self.schedule_backoff(task, now);
                        }
                    }
                    Err(AppSendError::SendError(send_err)) => {
                        log_debug!(
                            "[REPL][{}] push to {} send error: {:?} (kind={:?} target={:?})",
                            task.group_id,
                            task.peer,
                            send_err,
                            task.kind,
                            target
                        );
                        log_debug!(
                            "[REPL_DIAG][{}] push send_err after_ms={} kind={:?} peer={}",
                            task.group_id,
                            rpc_started.elapsed().as_millis(),
                            task.kind,
                            task.peer
                        );
                        self.schedule_backoff(task, now);
                    }
                    Err(AppSendError::BuildError(build_err)) => {
                        log_debug!(
                            "[REPL][{}] push to {} build error: {:?} (kind={:?} target={:?})",
                            task.group_id,
                            task.peer,
                            build_err,
                            task.kind,
                            target
                        );
                        log_debug!(
                            "[REPL_DIAG][{}] push build_err after_ms={} kind={:?} peer={}",
                            task.group_id,
                            rpc_started.elapsed().as_millis(),
                            task.kind,
                            task.peer
                        );
                        self.schedule_backoff(task, now);
                    }
                }
            }
            ReplicationKind::PullSnapshot => {
                if let Some(res) = self
                    .fetch_snapshot_from_peer(&task.group_id, &task.peer)
                    .await
                {
                    if let Err(err) = self.apply_group_update_payload(
                        &task.group_id,
                        &res.update_payload,
                        "replication_pull_snapshot",
                        None,
                        false,
                    ) {
                        log_debug!(
                            "[REPL][{}] failed to apply snapshot from {}: {}",
                            task.group_id,
                            task.peer,
                            err
                        );
                        self.schedule_backoff(task, now);
                    } else if self.local_group_acl_ready(&task.group_id) {
                        self.groups_pending_bootstrap.remove(&task.group_id);
                    }
                } else {
                    self.schedule_backoff(task, now);
                }
            }
            ReplicationKind::PullDelta => {
                let sv = self
                    .group_doc_managers
                    .get(&task.group_id)
                    .and_then(|mgr| mgr.last_state_vector().cloned());
                if let Some(res) = self
                    .fetch_update_from_peer(&task.group_id, &task.peer, sv)
                    .await
                {
                    if res.update_payload.is_empty() {
                        return;
                    }
                    if let Err(err) = self.apply_group_update_payload(
                        &task.group_id,
                        &res.update_payload,
                        "replication_pull_delta",
                        None,
                        false,
                    ) {
                        log_debug!(
                            "[REPL][{}] failed to apply delta from {}: {}",
                            task.group_id,
                            task.peer,
                            err
                        );
                        self.schedule_backoff(task, now);
                    }
                } else {
                    self.schedule_backoff(task, now);
                }
            }
        }
    }

    fn schedule_backoff(&mut self, mut task: ReplicationTask, now: u64) {
        let delay = (1u64 << (task.attempt.min(6))) * 2;
        task.attempt = task.attempt.saturating_add(1);
        task.not_before = now + delay;
        self.replication_metrics.retries = self.replication_metrics.retries.saturating_add(1);
        log_debug!(
            "[REPL][{}] backoff {:?} to {} (attempt {} delay={}s)",
            task.group_id,
            task.kind,
            task.peer,
            task.attempt,
            delay
        );
        self.enqueue_replication_task(task);
    }

    fn enqueue_bootstrap_pulls(&mut self, now: u64) {
        let pending: Vec<GroupId> = self
            .groups
            .keys()
            .cloned()
            .filter(|g| self.group_needs_bootstrap(g))
            .collect();
        if !pending.is_empty() {
            log_debug!(
                "[BOOT] enqueue_bootstrap_pulls pending_groups={:?}",
                pending
            );
        }
        for group_id in pending {
            let peers: Vec<String> = self
                .groups
                .get(&group_id)
                .map(|g| {
                    g.hubs
                        .active
                        .iter()
                        .filter(|p| *p != &our().node)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            for peer in peers {
                if self.has_replication_task(&group_id, &peer, ReplicationKind::PullSnapshot) {
                    continue;
                }
                self.enqueue_replication_task(ReplicationTask {
                    group_id: group_id.clone(),
                    peer,
                    kind: ReplicationKind::PullSnapshot,
                    since: None,
                    attempt: 0,
                    not_before: now,
                });
            }
        }
    }

    async fn fetch_update_from_peer(
        &self,
        group_id: &GroupId,
        peer: &str,
        state_vector: Option<StateVector>,
    ) -> Option<CrdtUpdateRes> {
        let target = Address::from((peer, OUR_PROCESS_ID));
        let sv_encoded = state_vector
            .as_ref()
            .map(|sv| base64_encode(&sv.encode_v1()));
        let body = serde_json::to_vec(&serde_json::json!({
            "CrdtGroupUpdate": {
                "group_id": group_id,
                "state_vector": sv_encoded,
            }
        }))
        .ok()?;
        let request = Request::new()
            .target(target.clone())
            .body(body)
            .expects_response(REPL_RPC_TIMEOUT_SECS);
        log_debug!(
            "[REPL][{}] fetch_update_from_peer peer={} target={}",
            group_id,
            peer,
            target
        );

        let req_started = Instant::now();
        match send::<serde_json::Value>(request).await {
            Ok(val) => {
                let res = val
                    .get("Ok")
                    .cloned()
                    .or_else(|| Some(val.clone()))
                    .and_then(|v| serde_json::from_value::<CrdtUpdateRes>(v).ok());
                if let Some(res) = res {
                    log_debug!(
                        "[REPL_DIAG][{}] fetch_update_from_peer ok peer={} elapsed_ms={}",
                        group_id,
                        peer,
                        req_started.elapsed().as_millis(),
                    );
                    Some(res)
                } else {
                    log_debug!(
                        "[REPL][{}] failed to decode delta from {} body={:?}",
                        group_id,
                        peer,
                        val
                    );
                    None
                }
            }
            Err(AppSendError::SendError(err)) => {
                log_debug!(
                    "[REPL][{}] failed to fetch delta from {} send_err={:?}",
                    group_id,
                    peer,
                    err
                );
                log_debug!(
                    "[REPL_DIAG][{}] fetch_update_from_peer err peer={} elapsed_ms={}",
                    group_id,
                    peer,
                    req_started.elapsed().as_millis()
                );
                None
            }
            Err(AppSendError::BuildError(build_err)) => {
                log_debug!(
                    "[REPL][{}] failed to build delta request to {}: {:?}",
                    group_id,
                    peer,
                    build_err
                );
                None
            }
        }
    }

    async fn fetch_snapshot_from_peer(
        &self,
        group_id: &GroupId,
        peer: &str,
    ) -> Option<CrdtUpdateRes> {
        let target = Address::from((peer, OUR_PROCESS_ID));
        let body = serde_json::to_vec(&serde_json::json!({
            "CrdtGroupSnapshot": { "group_id": group_id }
        }))
        .ok()?;
        let request = Request::new()
            .target(target.clone())
            .body(body)
            // Keep snapshot pulls within the replication_work RPC budget.
            .expects_response(REPL_RPC_TIMEOUT_SECS);
        log_debug!(
            "[REPL][{}] fetch_snapshot_from_peer peer={} target={}",
            group_id,
            peer,
            target
        );

        let req_started = Instant::now();
        match send::<serde_json::Value>(request).await {
            Ok(val) => {
                let res = val
                    .get("Ok")
                    .cloned()
                    .or_else(|| Some(val.clone()))
                    .and_then(|v| serde_json::from_value::<CrdtUpdateRes>(v).ok());
                if let Some(res) = res {
                    log_debug!(
                        "[REPL_DIAG][{}] fetch_snapshot_from_peer ok peer={} elapsed_ms={}",
                        group_id,
                        peer,
                        req_started.elapsed().as_millis(),
                    );
                    Some(res)
                } else {
                    log_debug!(
                        "[REPL][{}] failed to decode snapshot from {} body={:?}",
                        group_id,
                        peer,
                        val
                    );
                    None
                }
            }
            Err(AppSendError::SendError(err)) => {
                log_debug!(
                    "[REPL][{}] failed to fetch snapshot from {} send_err={:?}",
                    group_id,
                    peer,
                    err
                );
                log_debug!(
                    "[REPL_DIAG][{}] fetch_snapshot_from_peer err peer={} elapsed_ms={}",
                    group_id,
                    peer,
                    req_started.elapsed().as_millis()
                );
                None
            }
            Err(AppSendError::BuildError(build_err)) => {
                log_debug!(
                    "[REPL][{}] failed to build snapshot request to {}: {:?}",
                    group_id,
                    peer,
                    build_err
                );
                None
            }
        }
    }

    fn apply_group_update_payload(
        &mut self,
        group_id: &GroupId,
        update_payload: &str,
        context: &str,
        incoming_acl_version: Option<u64>,
        is_subscriber_lane: bool,
    ) -> Result<(), String> {
        let local_has_access = self.local_group_acl_ready(group_id);
        let local_member_status = self
            .groups
            .get(group_id)
            .and_then(|g| g.members.get(&our().node).map(|m| m.status));
        log_debug!(
            "[CRDT][{}] apply_group_update_payload: context={} len={} local_acl_ready={} pending_bootstrap={} is_sub_lane={} incoming_acl={:?} local_member_status={:?}",
            group_id,
            context,
            update_payload.len(),
            local_has_access,
            self.group_needs_bootstrap(group_id),
            is_subscriber_lane,
            incoming_acl_version,
            local_member_status
        );
        if let Some(in_acl) = incoming_acl_version {
            if let Some(local_wl) = self.pubsub.whitelist(group_id) {
                let local_version = local_wl.version();
                if in_acl != local_version {
                    log_debug!(
                        "[CRDT][{}] ACL version drift: incoming={} local={}",
                        group_id,
                        in_acl,
                        local_version
                    );
                }
            }
        }

        let update_bytes = base64_decode(update_payload.trim())
            .map_err(|e| format!("Invalid update payload: {e}"))?;
        if update_bytes.is_empty() {
            log_debug!(
                "[CRDT][{}] context={} received EMPTY update payload",
                group_id,
                context
            );
        }
        let was_missing = !self.groups.contains_key(group_id);
        if was_missing {
            self.groups_pending_bootstrap.insert(group_id.clone());
            log_debug!(
                "[BOOT] new group seen via {} -> added to pending_bootstrap set",
                context
            );
        }

        if update_bytes.is_empty() && (was_missing || self.group_needs_bootstrap(group_id)) {
            log_debug!(
                "[CRDT][{}] context={} skipping empty update during bootstrap",
                group_id,
                context
            );
            return Ok(());
        }

        let mut enforce_acl = local_has_access && !self.group_needs_bootstrap(group_id);
        if enforce_acl {
            let local_status = self
                .groups
                .get(group_id)
                .and_then(|g| g.members.get(&our().node).map(|m| m.status));
            if !matches!(local_status, Some(MembershipStatus::Active)) {
                // Allow membership bootstrap/update to proceed even if we're not yet whitelisted.
                log_debug!(
                    "[CRDT][{}] bypassing ACL for local_status={:?} context={}",
                    group_id,
                    local_status,
                    context
                );
                enforce_acl = false;
            }
        }
        if enforce_acl {
            let routing = self
                .groups
                .get(group_id)
                .map(|g| g.routing.clone())
                .unwrap_or_default();
            let routing_unavailable =
                routing.hub_topic.is_empty() && routing.subscriber_topic.is_empty();
            if !routing_unavailable {
                // Accept either hub subscription or subscriber subscription depending on lane + role.
                let hub_ok = self.require_hub_subscription(group_id, &our().node);
                if let Err(hub_err) = hub_ok {
                    let sub_ok = self.require_subscriber_access(group_id, &our().node);
                    if let Err(sub_err) = sub_ok {
                        // Only allow bypass when this update arrived via subscriber lane and we're not yet active.
                        let member_status = self
                            .groups
                            .get(group_id)
                            .and_then(|g| g.members.get(&our().node).map(|m| m.status));
                        let is_new_or_pending = member_status.is_none()
                            || matches!(member_status, Some(MembershipStatus::Pending));
                        if !(is_subscriber_lane && is_new_or_pending) {
                            log_debug!(
                                "[CRDT][{}] ACL reject context={} hub_err={} sub_err={} member_status={:?} is_sub_lane={}",
                                group_id,
                                context,
                                hub_err,
                                sub_err,
                                member_status,
                                is_subscriber_lane
                            );
                            return Err(format!(
                                "hub subscription denied: {}; subscriber access denied: {}",
                                hub_err, sub_err
                            ));
                        }
                    }
                }
            }
        }

        let manager = self
            .ensure_group_doc_manager(group_id)
            .map_err(|e| format!("Failed to init group CRDT: {:?}", e))?;

        let doc_id = manager.doc().id().to_string();
        log_debug!(
            "[CRDT][{}] context={} incoming_update_bytes={}",
            doc_id,
            context,
            update_bytes.len()
        );

        {
            let doc = manager.doc();
            doc.apply_update(&update_bytes)
                .map_err(|e| format!("Failed to apply update: {:?}", e))?;
        }

        let group_state = {
            let doc = manager.doc();
            doc.read_state()
                .map_err(|e| format!("Failed to read CRDT state: {:?}", e))?
        };
        log_group_state_summary(&doc_id, context, &group_state);
        log_debug!(
            "[CRDT][{}] context={} members_detail={:?}",
            doc_id,
            context,
            group_state
                .group
                .members
                .iter()
                .map(|(k, v)| (k, (&v.role_id, v.status)))
                .collect::<Vec<_>>()
        );
        log_debug!(
            "[CRDT][{}] context={} applied_ok members={} hubs={} subs={}",
            doc_id,
            context,
            group_state.group.members.len(),
            group_state.group.hubs.active.len(),
            group_state.group.subscribers.entries.len()
        );

        let new_vector = {
            let doc = manager.doc();
            doc.state_vector()
        };
        log_crdt_event(&doc_id, context, &new_vector, Some(update_bytes.len()));
        manager.set_last_state_vector(new_vector.clone());
        self.update_local_hub_sync_state(group_id, &new_vector);

        // Capture old message IDs before applying update
        let old_message_ids: std::collections::HashSet<String> = self
            .groups
            .get(group_id)
            .map(|g| g.messages.keys().cloned().collect())
            .unwrap_or_default();

        group_state.apply_into(self);
        self.rebuild_group_search(group_id);

        // Detect new messages and handle notifications/unread counts
        if let Some(group) = self.groups.get(group_id) {
            let group_name = group
                .metadata
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "Group".to_string());
            let our_node = our().node.clone();

            // Find new messages from other users
            let new_messages: Vec<_> = group
                .messages
                .values()
                .filter(|m| !old_message_ids.contains(&m.message_id) && m.sender != our_node)
                .collect();

            if !new_messages.is_empty() {
                // Increment unread count for this group
                let unread_increment = new_messages.len() as u32;
                *self.group_unread.entry(group_id.clone()).or_insert(0) += unread_increment;

                // Send push notification if conditions are met
                let group_notify_enabled = self.group_notify.get(group_id).copied().unwrap_or(true);
                let global_notify_enabled = self.settings.notify_groups;
                let active_connection_count = self.active_connections.len();
                log_debug!(
                    "[NOTIFY] group_push_gate group_id={} group_notify={} global_notify={} active_connections={}",
                    group_id,
                    group_notify_enabled,
                    global_notify_enabled,
                    active_connection_count
                );
                if global_notify_enabled && group_notify_enabled && active_connection_count == 0 {
                    // Only notify for the most recent message to avoid spam
                    if let Some(latest) = new_messages.iter().max_by_key(|m| m.timestamp) {
                        let sender = latest.sender.clone();
                        let content = latest.body.clone();
                        let gid = group_id.clone();
                        let gname = group_name.clone();
                        spawn(async move {
                            send_push_notification_for_group_message(
                                &sender, &content, &gid, &gname,
                            )
                            .await;
                        });
                    }
                } else {
                    log_debug!(
                        "[NOTIFY] group_push_skip group_id={} group_notify={} global_notify={} active_connections={}",
                        group_id,
                        group_notify_enabled,
                        global_notify_enabled,
                        active_connection_count
                    );
                }
            }
        }

        // Notify browser clients so they can refresh group state after receiving remote updates.
        self.broadcast_ws_message(&WsServerMessage::GroupUpdate {
            group_id: group_id.clone(),
        });

        // Consider bootstrap complete once the local node is an active member (or otherwise ACL-ready).
        // Also mark complete if the local member has been removed - no point bootstrapping a group
        // we've been kicked from.
        let local_member = self
            .groups
            .get(group_id)
            .and_then(|g| g.members.get(&our().node));
        let has_local_membership = local_member
            .map(|m| m.status == MembershipStatus::Active)
            .unwrap_or(false);
        let is_removed = local_member
            .map(|m| m.status == MembershipStatus::Removed)
            .unwrap_or(false);

        let acl_ready = self.local_group_acl_ready(group_id);
        log_debug!(
            "[CRDT][{}] context={} post-apply has_local_membership={} is_removed={} acl_ready={} pending_bootstrap={}",
            group_id,
            context,
            has_local_membership,
            is_removed,
            acl_ready,
            self.group_needs_bootstrap(group_id)
        );
        if acl_ready || has_local_membership || is_removed {
            self.mark_group_bootstrapped(group_id);
        }
        Ok(())
    }
}

// Helper methods implementation
impl ChatState {
    fn infer_counterparty_from_chat_id(chat_id: &str, our_node: &str) -> String {
        let mut parts = chat_id.splitn(2, ':');
        let first = parts.next().unwrap_or_default();
        let second = parts.next().unwrap_or_default();

        if first == our_node {
            second.to_string()
        } else if second == our_node {
            first.to_string()
        } else {
            // malformed ID; fall back to the tail for now
            second.to_string()
        }
    }

    // Normalize chat ID to prevent duplicates
    // Always returns the ID in alphabetical order: "nodeA:nodeB"
    fn normalize_chat_id(node1: &str, node2: &str) -> String {
        if node1 < node2 {
            format!("{}:{}", node1, node2)
        } else {
            format!("{}:{}", node2, node1)
        }
    }

    fn reconcile_dm_chats_for_counterparty(&mut self, counterparty_node: &str) -> bool {
        if counterparty_node.is_empty() || counterparty_node == our().node {
            return false;
        }

        let canonical_chat_id = Self::normalize_chat_id(counterparty_node, &our().node);
        let mut candidate_ids: Vec<String> = self
            .chats
            .iter()
            .filter_map(|(chat_id, chat)| {
                if chat_id.starts_with("system:") || chat_id.starts_with("browser:") {
                    return None;
                }

                let inferred_counterparty =
                    Self::infer_counterparty_from_chat_id(chat_id, &our().node);
                let has_messages_from_counterparty = chat
                    .messages
                    .iter()
                    .any(|message| message.sender == counterparty_node);

                if chat_id == &canonical_chat_id
                    || chat.counterparty == counterparty_node
                    || inferred_counterparty == counterparty_node
                    || has_messages_from_counterparty
                {
                    Some(chat_id.clone())
                } else {
                    None
                }
            })
            .collect();

        if candidate_ids.is_empty() {
            return false;
        }

        candidate_ids.sort();
        candidate_ids.dedup();

        if candidate_ids.len() == 1 && candidate_ids[0] == canonical_chat_id {
            let mut changed = false;
            if let Some(chat) = self.chats.get_mut(&canonical_chat_id) {
                if chat.counterparty != counterparty_node {
                    chat.counterparty = counterparty_node.to_string();
                    changed = true;
                }
                if let Some(profile) = self.node_profiles.get(counterparty_node).cloned() {
                    if chat.counterparty_profile.as_ref() != Some(&profile) {
                        chat.counterparty_profile = Some(profile);
                        changed = true;
                    }
                }
            }
            if changed {
                self.rebuild_chat_search(&canonical_chat_id);
            }
            return changed;
        }

        let mut removed_chat_ids: Vec<String> = Vec::new();
        let mut merged_chat = if let Some(chat) = self.chats.remove(&canonical_chat_id) {
            removed_chat_ids.push(canonical_chat_id.clone());
            chat
        } else {
            Chat {
                id: canonical_chat_id.clone(),
                counterparty: counterparty_node.to_string(),
                messages: Vec::new(),
                last_activity: 0,
                unread_count: 0,
                is_blocked: false,
                notify: true,
                counterparty_profile: self.node_profiles.get(counterparty_node).cloned(),
            }
        };

        let mut seen_message_ids: HashSet<String> = merged_chat
            .messages
            .iter()
            .map(|message| message.id.clone())
            .collect();

        for chat_id in candidate_ids {
            if chat_id == canonical_chat_id {
                continue;
            }

            if let Some(alias_chat) = self.chats.remove(&chat_id) {
                removed_chat_ids.push(chat_id.clone());
                self.message_sequence_counters.remove(&chat_id);

                merged_chat.last_activity = merged_chat.last_activity.max(alias_chat.last_activity);
                merged_chat.unread_count = merged_chat.unread_count.max(alias_chat.unread_count);
                merged_chat.is_blocked = merged_chat.is_blocked || alias_chat.is_blocked;
                merged_chat.notify = merged_chat.notify && alias_chat.notify;
                if merged_chat.counterparty_profile.is_none() {
                    merged_chat.counterparty_profile = alias_chat.counterparty_profile.clone();
                }

                for message in alias_chat.messages {
                    if seen_message_ids.insert(message.id.clone()) {
                        merged_chat.messages.push(message);
                    }
                }
            }
        }

        merged_chat.id = canonical_chat_id.clone();
        merged_chat.counterparty = counterparty_node.to_string();
        if let Some(profile) = self.node_profiles.get(counterparty_node).cloned() {
            merged_chat.counterparty_profile = Some(profile);
        }
        merged_chat.messages.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then_with(|| {
                    a.sequence
                        .unwrap_or(u64::MAX)
                        .cmp(&b.sequence.unwrap_or(u64::MAX))
                })
                .then_with(|| a.id.cmp(&b.id))
        });

        self.chats.insert(canonical_chat_id.clone(), merged_chat);
        self.message_sequence_counters.remove(&canonical_chat_id);
        self.ensure_sequence_state(&canonical_chat_id);

        for removed_chat_id in removed_chat_ids {
            self.search_index.remove_chat(&removed_chat_id);
        }
        self.rebuild_chat_search(&canonical_chat_id);

        true
    }

    fn is_hex(s: &str) -> bool {
        s.chars().all(|ch| ch.is_ascii_hexdigit())
    }

    fn is_valid_tx_hash(tx_hash: &str) -> bool {
        let Some(hash) = tx_hash.strip_prefix("0x") else {
            return false;
        };
        hash.len() == 64 && Self::is_hex(hash)
    }

    fn is_valid_evm_address(address: &str) -> bool {
        let Some(hex) = address.strip_prefix("0x") else {
            return false;
        };
        hex.len() == 40 && Self::is_hex(hex)
    }

    fn contacts_process_address() -> Address {
        Address::from((our().node.as_str(), CONTACTS_PROCESS_ID))
    }

    fn contacts_capability(params: &str) -> Capability {
        Capability::new(&Self::contacts_process_address(), format!("\"{params}\""))
    }

    fn sync_contact_profile_fields(node: String, profile: UserProfile) {
        let contacts_process = Self::contacts_process_address();
        spawn(async move {
            let add_cap = Self::contacts_capability("Add");
            let remove_cap = Self::contacts_capability("Remove");
            let caps = vec![add_cap, remove_cap];

            let nickname_value = serde_json::Value::String(profile.name);
            let nickname_body = serde_json::json!({
                "AddField": [node.clone(), CONTACTS_FIELD_NICKNAME, nickname_value.to_string()]
            });
            if let Ok(body) = serde_json::to_vec(&nickname_body) {
                let _ = Request::to(&contacts_process)
                    .body(body)
                    .capabilities(caps.clone())
                    .send_and_await_response(5);
            }

            if let Some(base_address) = profile.base_address {
                let base_value = serde_json::Value::String(base_address);
                let base_body = serde_json::json!({
                    "AddField": [node.clone(), CONTACTS_FIELD_BASE_ADDRESS, base_value.to_string()]
                });
                if let Ok(body) = serde_json::to_vec(&base_body) {
                    let _ = Request::to(&contacts_process)
                        .body(body)
                        .capabilities(caps)
                        .send_and_await_response(5);
                }
            } else {
                let remove_body = serde_json::json!({
                    "RemoveField": [node, CONTACTS_FIELD_BASE_ADDRESS]
                });
                if let Ok(body) = serde_json::to_vec(&remove_body) {
                    let _ = Request::to(&contacts_process)
                        .body(body)
                        .capabilities(caps)
                        .send_and_await_response(5);
                }
            }
        });
    }

    fn get_or_create_chat<'a>(
        &'a mut self,
        chat_id: &str,
        timestamp: u64,
        counterparty_hint: Option<String>,
        profile_hint: Option<UserProfile>,
    ) -> &'a mut Chat {
        if !self.chats.contains_key(chat_id) {
            let counterparty = counterparty_hint
                .or_else(|| Some(Self::infer_counterparty_from_chat_id(chat_id, &our().node)))
                .unwrap_or_else(|| chat_id.to_string());

            let profile = profile_hint.or_else(|| self.node_profiles.get(&counterparty).cloned());

            self.chats.insert(
                chat_id.to_string(),
                Chat {
                    id: chat_id.to_string(),
                    counterparty: counterparty.clone(),
                    messages: Vec::new(),
                    last_activity: timestamp,
                    unread_count: 0,
                    is_blocked: false,
                    notify: true,
                    counterparty_profile: profile,
                },
            );
            self.message_sequence_counters
                .entry(chat_id.to_string())
                .or_insert(0);
        }

        self.chats
            .get_mut(chat_id)
            .expect("chat must exist after get_or_create_chat")
    }

    fn ensure_sequence_state(&mut self, chat_id: &str) {
        if self.message_sequence_counters.contains_key(chat_id) {
            return;
        }

        if !self.chats.contains_key(chat_id) {
            self.message_sequence_counters
                .insert(chat_id.to_string(), 0);
            return;
        }

        let chat = self
            .chats
            .get_mut(chat_id)
            .expect("chat must exist when ensuring sequences");

        let mut next_seq = 0u64;
        let mut indices: Vec<usize> = (0..chat.messages.len()).collect();
        indices.sort_by(|&i, &j| {
            chat.messages[i]
                .timestamp
                .cmp(&chat.messages[j].timestamp)
                .then_with(|| chat.messages[i].id.cmp(&chat.messages[j].id))
        });

        let mut max_existing = chat
            .messages
            .iter()
            .filter_map(|m| m.sequence)
            .max()
            .map(|val| val + 1);

        if max_existing.is_some() {
            next_seq = max_existing.take().unwrap();
        }

        for idx in indices {
            if chat.messages[idx].sequence.is_none() {
                chat.messages[idx].sequence = Some(next_seq);
                next_seq += 1;
            }
        }

        if next_seq == 0 {
            next_seq = chat
                .messages
                .iter()
                .filter_map(|m| m.sequence)
                .max()
                .map(|val| val + 1)
                .unwrap_or(0);
        }

        self.message_sequence_counters
            .insert(chat_id.to_string(), next_seq);
    }

    fn assign_sequence_to_message(&mut self, chat_id: &str, message: &mut ChatMessage) {
        if message.sequence.is_some() {
            return;
        }

        self.ensure_sequence_state(chat_id);

        if let Some(counter) = self.message_sequence_counters.get_mut(chat_id) {
            let seq = *counter;
            *counter += 1;
            message.sequence = Some(seq);
        }
    }

    fn enqueue_delivery_message(&self, node: &str, message: ChatMessage) {
        ChatState::enqueue_delivery_message_inner(
            &self.delivery_tx,
            &self.pending_deliveries,
            node,
            message,
        );
    }

    fn enqueue_delivery_flush(&self, node: &str) {
        if let Err(err) = self
            .delivery_tx
            .unbounded_send(QueuedDelivery::flush(node.to_string()))
        {
            log_debug!("Failed to enqueue delivery flush for {}: {:?}", node, err);
        }
    }

    fn bootstrap_pending_deliveries(&self) {
        for chat in self.chats.values() {
            let counterparty = chat.counterparty.clone();
            for message in chat.messages.iter() {
                if message.sender == our().node
                    && matches!(
                        message.status,
                        MessageStatus::Sent | MessageStatus::Sending | MessageStatus::Failed
                    )
                {
                    self.enqueue_delivery_message(&counterparty, message.clone());
                }
            }
        }
    }

    fn enqueue_delivery_message_inner(
        delivery_tx: &DeliveryTx,
        pending_deliveries: &Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
        node: &str,
        message: ChatMessage,
    ) {
        ChatState::record_pending_message(pending_deliveries, node, &message);
        if let Err(err) =
            delivery_tx.unbounded_send(QueuedDelivery::message(node.to_string(), message))
        {
            log_debug!("Failed to enqueue delivery message for {}: {:?}", node, err);
        }
    }

    fn record_pending_message(
        pending: &Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
        node: &str,
        message: &ChatMessage,
    ) {
        let mut guard = pending.lock().unwrap();
        let entry = guard.entry(node.to_string()).or_default();
        if let Some(existing) = entry.iter_mut().find(|m| m.id == message.id) {
            *existing = message.clone();
        } else {
            entry.push(message.clone());
        }
    }

    fn remove_pending_message(
        pending: &Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
        node: &str,
        message_id: &str,
    ) {
        let mut guard = pending.lock().unwrap();
        if let Some(entry) = guard.get_mut(node) {
            entry.retain(|m| m.id != message_id);
            if entry.is_empty() {
                guard.remove(node);
            }
        }
    }

    fn pending_messages_for_node(
        pending: &Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
        node: &str,
    ) -> Vec<ChatMessage> {
        let guard = pending.lock().unwrap();
        guard.get(node).cloned().unwrap_or_default()
    }

    fn is_message_pending(
        pending: &Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
        node: &str,
        message_id: &str,
    ) -> bool {
        let guard = pending.lock().unwrap();
        guard
            .get(node)
            .map(|messages| messages.iter().any(|m| m.id == message_id))
            .unwrap_or(false)
    }

    async fn run_delivery_worker(
        mut delivery_rx: UnboundedReceiver<QueuedDelivery>,
        delivery_tx: DeliveryTx,
        pending_deliveries: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    ) {
        while let Some(queued) = delivery_rx.next().await {
            match queued.event {
                DeliveryEvent::Message(message) => {
                    ChatState::record_pending_message(&pending_deliveries, &queued.node, &message);
                    ChatState::attempt_delivery(
                        queued.node.clone(),
                        message,
                        delivery_tx.clone(),
                        pending_deliveries.clone(),
                    )
                    .await;
                }
                DeliveryEvent::Flush => {
                    let messages =
                        ChatState::pending_messages_for_node(&pending_deliveries, &queued.node);
                    for msg in messages {
                        if let Err(err) = delivery_tx
                            .unbounded_send(QueuedDelivery::message(queued.node.clone(), msg))
                        {
                            log_debug!(
                                "Failed to enqueue message during flush for {}: {:?}",
                                queued.node,
                                err
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn attempt_delivery(
        node: String,
        message: ChatMessage,
        delivery_tx: DeliveryTx,
        pending_deliveries: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    ) {
        let target = Address::from((node.as_str(), OUR_PROCESS_ID));

        let msg_json = serde_json::to_value(&message).unwrap();
        let msg_for_rpc: CUChatMessage = serde_json::from_value(msg_json).unwrap();

        match receive_message_remote_rpc(&target, msg_for_rpc).await {
            Ok(_) => {
                ChatState::remove_pending_message(&pending_deliveries, &node, &message.id);
            }
            Err(err) => {
                log_debug!(
                    "Failed to deliver message {} to {}: {:?}",
                    message.id,
                    node,
                    err
                );
                let retry_tx = delivery_tx.clone();
                let retry_pending = pending_deliveries.clone();
                let retry_node = node.clone();
                let retry_message = message.clone();
                spawn(async move {
                    let _ = sleep(30000).await;
                    if ChatState::is_message_pending(&retry_pending, &retry_node, &retry_message.id)
                    {
                        if let Err(send_err) = retry_tx.unbounded_send(QueuedDelivery::message(
                            retry_node.clone(),
                            retry_message,
                        )) {
                            log_debug!(
                                "Failed to requeue message {} for {}: {:?}",
                                message.id,
                                retry_node,
                                send_err
                            );
                        }
                    }
                });
            }
        }
    }
}

// Helper functions for converting between UserProfile types
impl ChatState {
    // Helper function to convert our UserProfile to chat_caller_utils::UserProfile
    fn to_cu_user_profile(profile: &UserProfile) -> CUUserProfile {
        CUUserProfile {
            name: profile.name.clone(),
            profile_pic: profile.profile_pic.clone(),
            base_address: profile.base_address.clone(),
        }
    }

    /// Validates a Spider API key by making a lightweight test request
    async fn validate_spider_key(&self, api_key: &str) -> bool {
        const SPIDER_PROCESS_ID: (&str, &str, &str) = ("spider", "spider", "sys");

        log_debug!(
            "[SPIDER] validate_spider_key called for key: {}...",
            &api_key[..8.min(api_key.len())]
        );

        let body = serde_json::json!({
            "ListMcpServers": {
                "authKey": api_key,
            }
        });

        let request = ProcessRequest::to(Address::new("our", SPIDER_PROCESS_ID))
            .body(match serde_json::to_vec(&body) {
                Ok(b) => b,
                Err(e) => {
                    log_debug!("[SPIDER] Failed to serialize validation request: {}", e);
                    return false;
                }
            })
            .expects_response(5);

        let result: Result<serde_json::Value, _> = hyperapp::send(request).await;
        log_debug!("[SPIDER] Validation result: {:?}", result);

        match result {
            Ok(json_body) => {
                // Check if response is an error
                if let Some(err) = json_body.get("Err") {
                    let err_str = err.as_str().unwrap_or("");
                    let is_valid =
                        !err_str.contains("Unauthorized") && !err_str.contains("Invalid API key");
                    log_debug!(
                        "[SPIDER] Validation response has Err: {}, is_valid: {}",
                        err_str,
                        is_valid
                    );
                    // If unauthorized or invalid key, return false
                    is_valid
                } else {
                    log_debug!("[SPIDER] Validation successful (no Err in response)");
                    true
                }
            }
            Err(e) => {
                log_debug!("[SPIDER] Validation request failed: {:?}", e);
                false
            }
        }
    }
}
