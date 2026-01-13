use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crdt::{
    Group, GroupId, GroupTier, MembershipActionKind, MembershipDecision, MembershipDecisionStatus,
    MembershipProposal, MembershipRuleBox, MembershipRuleConfig, MembershipStatus, NodeId,
    SubscriberSyncState, ThreadId,
};
use hyperware_process_lib::our;

/// Group/membership helpers shared across state and group modules.
pub(crate) fn aggregate_rule_decisions(
    rules: &[MembershipRuleBox],
    proposal: &MembershipProposal,
) -> MembershipDecision {
    if rules.is_empty() {
        return MembershipDecision::approved();
    }

    let mut pending: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for rule in rules {
        let decision = rule.evaluate(proposal);
        match decision.status {
            MembershipDecisionStatus::Approved => {}
            MembershipDecisionStatus::Pending => {
                for sig in decision.missing_signatures {
                    pending.insert(sig);
                }
            }
            MembershipDecisionStatus::Rejected => return decision,
        }
    }

    if pending.is_empty() {
        MembershipDecision::approved()
    } else {
        let mut missing: Vec<NodeId> = pending.into_iter().collect();
        missing.sort();
        MembershipDecision::pending(missing)
    }
}

pub(crate) fn active_member_count(group: &Group) -> u32 {
    group
        .members
        .values()
        .filter(|member| member.status == MembershipStatus::Active)
        .count() as u32
}

pub(crate) fn membership_proposal_key(
    group_id: &GroupId,
    candidate: &NodeId,
    action: MembershipActionKind,
) -> String {
    let action_str = match action {
        MembershipActionKind::Invite => "invite",
        MembershipActionKind::Remove => "remove",
    };
    format!("{group_id}:{action_str}:{candidate}")
}

pub(crate) fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn ensure_membership_rules(
    mut rules: Vec<MembershipRuleConfig>,
    creator: &NodeId,
) -> Vec<MembershipRuleConfig> {
    if rules.is_empty() {
        rules = default_membership_rules(creator);
    }
    rules
}

fn default_membership_rules(creator: &NodeId) -> Vec<MembershipRuleConfig> {
    vec![MembershipRuleConfig::new(
        "membership.rule.dictator",
        json!({ "dictator": creator }),
    )]
}

pub(crate) fn default_group_message_type() -> MessageType {
    MessageType::Text
}

pub(crate) fn group_root_thread_id(group: &Group) -> Option<ThreadId> {
    group
        .metadata
        .as_ref()
        .map(|metadata| metadata.root_thread_id.clone())
}

pub(crate) fn sync_member_membership_sets(group: &mut Group, member_id: &NodeId, timestamp: u64) {
    let Some(member) = group.members.get(member_id) else {
        group.hubs.active.remove(member_id);
        group.subscribers.entries.remove(member_id);
        return;
    };

    if member.status == MembershipStatus::Removed {
        group.hubs.active.remove(member_id);
        group.subscribers.entries.remove(member_id);
        return;
    }

    if let Some(role) = group.roles.get(&member.role_id) {
        match role.tier {
            GroupTier::Hub => {
                group.hubs.active.insert(member_id.clone());
            }
            GroupTier::Subscriber => {
                group.hubs.active.remove(member_id);
            }
        }
    }

    let state = group
        .subscribers
        .entries
        .entry(member_id.clone())
        .or_insert_with(SubscriberSyncState::default);
    state.last_seen_ts = timestamp;
}

pub(crate) fn generate_group_id() -> GroupId {
    let timestamp = current_timestamp();
    let nonce: u32 = rand::random();
    format!("group:{}:{}:{}", our().node, timestamp, nonce)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PushSubscription {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NotificationsAction {
    SendNotification {
        title: String,
        body: String,
        icon: Option<String>,
        data: Option<serde_json::Value>,
    },
    GetPublicKey,
    InitializeKeys,
    AddSubscription {
        subscription: PushSubscription,
    },
    RemoveSubscription {
        endpoint: String,
    },
    ClearSubscriptions,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum NotificationsResponse {
    NotificationSent,
    PublicKey(String),
    KeysInitialized,
    SubscriptionAdded,
    SubscriptionRemoved,
    SubscriptionsCleared,
    Err(String),
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
    #[serde(default)]
    pub sequence: Option<u64>,
    pub status: MessageStatus,
    pub reply_to: Option<String>,
    pub reactions: Vec<MessageReaction>,
    pub message_type: MessageType,
    pub file_info: Option<FileInfo>,
}

impl<'de> Deserialize<'de> for ChatMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ChatMessageV2 {
            id: String,
            sender: String,
            content: String,
            timestamp: u64,
            #[serde(default)]
            sequence: Option<u64>,
            status: MessageStatus,
            reply_to: Option<String>,
            reactions: Vec<MessageReaction>,
            message_type: MessageType,
            file_info: Option<FileInfo>,
        }

        #[derive(Deserialize)]
        struct ChatMessageV1 {
            id: String,
            sender: String,
            content: String,
            timestamp: u64,
            status: MessageStatus,
            reply_to: Option<String>,
            reactions: Vec<MessageReaction>,
            message_type: MessageType,
            file_info: Option<FileInfo>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ChatMessageCompat {
            V2(ChatMessageV2),
            V1(ChatMessageV1),
        }

        match ChatMessageCompat::deserialize(deserializer)? {
            ChatMessageCompat::V2(msg) => Ok(ChatMessage {
                id: msg.id,
                sender: msg.sender,
                content: msg.content,
                timestamp: msg.timestamp,
                sequence: msg.sequence,
                status: msg.status,
                reply_to: msg.reply_to,
                reactions: msg.reactions,
                message_type: msg.message_type,
                file_info: msg.file_info,
            }),
            ChatMessageCompat::V1(msg) => Ok(ChatMessage {
                id: msg.id,
                sender: msg.sender,
                content: msg.content,
                timestamp: msg.timestamp,
                sequence: None,
                status: msg.status,
                reply_to: msg.reply_to,
                reactions: msg.reactions,
                message_type: msg.message_type,
                file_info: msg.file_info,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MessageReaction {
    pub emoji: String,
    pub user: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MessageType {
    Text,
    Image,
    File,
    VoiceNote,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FileInfo {
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    /// Encoded file reference. Expected variants:
    /// - `data:<mime>;base64,<bytes>` for inline images.
    /// - `compressed:<base64>` for non-image payloads we compress before send.
    /// - `/files/<chat_id>/<file_id>` or other VFS paths once stored locally.
    /// Other strings are treated as opaque remote paths.
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileUrlKind<'a> {
    DataUrl(&'a str),
    CompressedBase64(&'a str),
    VfsPath(&'a str),
    Other(&'a str),
}

impl FileInfo {
    pub fn url_kind(&self) -> FileUrlKind<'_> {
        if let Some(rest) = self.url.strip_prefix("compressed:") {
            FileUrlKind::CompressedBase64(rest)
        } else if self.url.starts_with("data:") {
            FileUrlKind::DataUrl(&self.url)
        } else if self.url.starts_with("/files/") {
            FileUrlKind::VfsPath(&self.url)
        } else {
            FileUrlKind::Other(&self.url)
        }
    }
}

/// MessageStatus lifecycle: outbound messages start at `Sending`, flip to `Sent`
/// once stored locally, move to `Delivered` on ack/receipt from the counterparty,
/// and may be marked `Failed` by delivery retries if a destination remains unreachable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chat {
    pub id: String,
    pub counterparty: String,
    pub messages: Vec<ChatMessage>,
    pub last_activity: u64,
    pub unread_count: u32,
    pub is_blocked: bool,
    pub notify: bool,
    #[serde(default)]
    pub counterparty_profile: Option<UserProfile>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatKey {
    pub key: String,
    pub user_name: String,
    pub created_at: u64,
    pub is_revoked: bool,
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GroupJoinKey {
    pub key: String,
    pub group_id: GroupId,
    pub created_at: u64,
    pub is_revoked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct UserProfile {
    pub name: String,
    pub profile_pic: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    pub show_images: bool,
    pub show_profile_pics: bool,
    pub combine_chats_groups: bool,
    pub notify_chats: bool,
    pub notify_groups: bool,
    pub notify_calls: bool,
    pub allow_browser_chats: bool,
    pub stt_enabled: bool,
    pub stt_api_key: Option<String>,
    pub max_file_size_mb: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_images: true,
            show_profile_pics: true,
            combine_chats_groups: false,
            notify_chats: true,
            notify_groups: true,
            notify_calls: true,
            allow_browser_chats: true,
            stt_enabled: false,
            stt_api_key: None,
            max_file_size_mb: 10,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsClientMessage {
    SendMessage {
        chat_id: String,
        content: String,
        reply_to: Option<String>,
    },
    Ack {
        message_id: String,
    },
    MarkRead {
        chat_id: String,
    },
    MarkGroupRead {
        group_id: String,
    },
    UpdateStatus {
        status: String,
    },
    AuthWithKey {
        chat_key: String,
    },
    BrowserMessage {
        content: String,
    },
    Heartbeat,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsServerMessage {
    NewMessage(ChatMessage),
    MessageAck {
        message_id: String,
    },
    StatusUpdate {
        node: String,
        status: String,
    },
    ChatUpdate(Chat),
    ProfileUpdate {
        node: String,
        profile: UserProfile,
    },
    AuthSuccess {
        chat_id: String,
        history: Vec<ChatMessage>,
    },
    AuthFailed {
        reason: String,
    },
    GroupUpdate {
        group_id: String,
    },
    Heartbeat,
    Error {
        message: String,
    },
}
