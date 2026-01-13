use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crdt::{
    Group, GroupCrdtManager, GroupId, GroupMember, GroupMetadata, GroupRoutingConfig, GroupTier,
    GroupVisibility, HubSyncState, MembershipRuleBox, MembershipStatus, NodeId, Role,
    SubscriberSyncState, Thread, ThreadParentRef,
};
use crate::pubsub::PubSubRegistry;
use crate::search::SearchIndex;
use hyperware_crdt::yrs::Encode;
use hyperware_crdt::CommitteeError;
use hyperware_process_lib::our;
use hyperware_pubsub_core::{whitelist::NodeId as BrokerNodeId, TopicId as BrokerTopicId};

use super::api::*;
use super::model::*;
use super::model::{current_timestamp, ensure_membership_rules, generate_group_id, group_root_thread_id};
use super::replication::{
    BrokerEnvelope, ReplicationKind, ReplicationMetrics, ReplicationTask, ReplicationTx,
    ReplicationWakeRx, ReplicationWakeTx, SubscriberDeliveryEvent,
};
use crate::WsServerMessage;

#[derive(Clone, Debug)]
pub enum DeliveryEvent {
    Message(ChatMessage),
    Flush,
}

#[derive(Clone, Debug)]
pub struct QueuedDelivery {
    pub node: String,
    pub event: DeliveryEvent,
}

impl QueuedDelivery {
    pub fn message(node: String, message: ChatMessage) -> Self {
        Self {
            node,
            event: DeliveryEvent::Message(message),
        }
    }

    pub fn flush(node: String) -> Self {
        Self {
            node,
            event: DeliveryEvent::Flush,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_state_runtime_fields_are_not_serialized() {
        let mut state = ChatState::default();
        state.profile.name = "alice".to_string();
        state.ws_connections.insert(1, "peer".to_string());
        state.browser_connections.insert("browser".to_string(), 2);
        state.last_heartbeat.insert(3, 123);
        state.active_connections.insert(4);
        state.node_profiles.insert(
            "peer".to_string(),
            UserProfile {
                name: "bob".into(),
                profile_pic: None,
            },
        );

        let value = serde_json::to_value(&state).expect("serialize ChatState");

        assert!(value.get("ws_connections").is_none());
        assert!(value.get("browser_connections").is_none());
        assert!(value.get("last_heartbeat").is_none());
        assert!(value.get("active_connections").is_none());
        assert!(value.get("node_profiles").is_none());

        let restored: ChatState = serde_json::from_value(value).expect("deserialize ChatState");

        assert_eq!(restored.profile.name, "alice");
        assert!(restored.ws_connections.is_empty());
        assert!(restored.browser_connections.is_empty());
        assert!(restored.last_heartbeat.is_empty());
        assert!(restored.active_connections.is_empty());
        assert!(restored.node_profiles.is_empty());
    }

    #[test]
    fn legacy_master_state_deserializes() {
        use serde::{Deserialize, Serialize};
        use std::collections::{HashMap, HashSet};

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        struct LegacyChatMessage {
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

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        struct LegacyChat {
            id: String,
            counterparty: String,
            messages: Vec<LegacyChatMessage>,
            last_activity: u64,
            unread_count: u32,
            is_blocked: bool,
            notify: bool,
            #[serde(default)]
            counterparty_profile: Option<UserProfile>,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        struct LegacyChatState {
            profile: UserProfile,
            chats: HashMap<String, LegacyChat>,
            chat_keys: HashMap<String, ChatKey>,
            settings: Settings,
            delivery_queue: HashMap<String, Vec<LegacyChatMessage>>,
            online_nodes: HashSet<String>,
            ws_connections: HashMap<u32, String>,
            browser_connections: HashMap<String, u32>,
            last_heartbeat: HashMap<u32, u64>,
            #[serde(default)]
            active_connections: HashSet<u32>,
            #[serde(default)]
            node_profiles: HashMap<String, UserProfile>,
        }

        let message = LegacyChatMessage {
            id: "123:1".to_string(),
            sender: "bob.node".to_string(),
            content: "hi".to_string(),
            timestamp: 123,
            status: MessageStatus::Delivered,
            reply_to: None,
            reactions: Vec::new(),
            message_type: MessageType::Text,
            file_info: None,
        };
        let chat = LegacyChat {
            id: "alice.node:bob.node".to_string(),
            counterparty: "bob.node".to_string(),
            messages: vec![message.clone()],
            last_activity: 123,
            unread_count: 0,
            is_blocked: false,
            notify: true,
            counterparty_profile: Some(UserProfile {
                name: "bob".to_string(),
                profile_pic: None,
            }),
        };
        let legacy = LegacyChatState {
            profile: UserProfile {
                name: "alice".to_string(),
                profile_pic: None,
            },
            chats: HashMap::from([(chat.id.clone(), chat)]),
            chat_keys: HashMap::new(),
            settings: Settings::default(),
            delivery_queue: HashMap::new(),
            online_nodes: HashSet::new(),
            ws_connections: HashMap::new(),
            browser_connections: HashMap::new(),
            last_heartbeat: HashMap::new(),
            active_connections: HashSet::new(),
            node_profiles: HashMap::from([(
                "bob.node".to_string(),
                UserProfile {
                    name: "bob".to_string(),
                    profile_pic: None,
                },
            )]),
        };

        let bytes = rmp_serde::to_vec(&legacy).expect("serialize legacy state via rmp");
        let restored: ChatState = rmp_serde::from_slice(&bytes).expect("deserialize into new state");

        assert_eq!(restored.profile.name, "alice");
        assert_eq!(restored.chats.len(), 1);
        let restored_chat = restored
            .chats
            .get("alice.node:bob.node")
            .expect("chat should deserialize");
        assert_eq!(restored_chat.messages.len(), 1);
        let restored_message = restored_chat.messages.first().unwrap();
        assert_eq!(restored_message.id, message.id);
        assert_eq!(restored_message.sender, message.sender);
        assert_eq!(restored_message.content, message.content);
        assert_eq!(restored_message.timestamp, message.timestamp);
        assert_eq!(restored_message.sequence, None);
        assert_eq!(restored_message.status, message.status);
        assert!(restored.groups.is_empty());
        assert_eq!(
            restored.node_profiles.get("bob.node").map(|p| p.name.as_str()),
            Some("bob")
        );
    }

    #[test]
    fn legacy_invite_only_groups_deserialize_as_public() {
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        enum LegacyGroupVisibility {
            Private,
            InviteOnly,
            Public,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        struct LegacyGroupMetadata {
            name: String,
            description: Option<String>,
            avatar: Option<String>,
            creator_id: String,
            created_at: u64,
            updated_at: u64,
            visibility: LegacyGroupVisibility,
            default_role_id: String,
            root_thread_id: String,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
        struct LegacyGroup {
            #[serde(default)]
            metadata: Option<LegacyGroupMetadata>,
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
        struct LegacyChatStateV2 {
            profile: UserProfile,
            chats: HashMap<String, Chat>,
            chat_keys: HashMap<String, ChatKey>,
            settings: Settings,
            message_sequence_counters: HashMap<String, u64>,
            groups: HashMap<GroupId, LegacyGroup>,
            group_unread: HashMap<GroupId, u32>,
            group_notify: HashMap<GroupId, bool>,
            node_profiles: HashMap<String, UserProfile>,
        }

        let group_id = "group:alice.node:123:0".to_string();
        let legacy_group = LegacyGroup {
            metadata: Some(LegacyGroupMetadata {
                name: "Legacy Group".to_string(),
                description: None,
                avatar: None,
                creator_id: "alice.node".to_string(),
                created_at: 100,
                updated_at: 200,
                visibility: LegacyGroupVisibility::InviteOnly,
                default_role_id: format!("{group_id}:member"),
                root_thread_id: format!("{group_id}:thread:root"),
            }),
        };

        let legacy = LegacyChatStateV2 {
            profile: UserProfile {
                name: "alice".to_string(),
                profile_pic: None,
            },
            chats: HashMap::new(),
            chat_keys: HashMap::new(),
            settings: Settings::default(),
            message_sequence_counters: HashMap::new(),
            groups: HashMap::from([(group_id.clone(), legacy_group)]),
            group_unread: HashMap::new(),
            group_notify: HashMap::new(),
            node_profiles: HashMap::new(),
        };

        let bytes = rmp_serde::to_vec(&legacy).expect("serialize legacy state via rmp");
        let restored: ChatState = rmp_serde::from_slice(&bytes).expect("deserialize into new state");

        let restored_group = restored
            .groups
            .get(&group_id)
            .expect("group should deserialize");
        let restored_visibility = restored_group
            .metadata
            .as_ref()
            .map(|meta| meta.visibility);
        assert_eq!(restored_visibility, Some(GroupVisibility::Public));
        assert!(restored.group_join_keys.is_empty());
    }
}

#[derive(Clone)]
pub struct DeliveryTx {
    sender: UnboundedSender<QueuedDelivery>,
}

/// Persisted fields: profile, chats, chat_keys, group_join_keys, settings, message_sequence_counters, groups.
/// Runtime-only state (connections, heartbeats, channels, replication queues, caches, pubsub) is
/// skipped during serialization and rebuilt on startup.
#[derive(Serialize)]
pub struct ChatState {
    pub profile: UserProfile,
    pub chats: HashMap<String, Chat>,
    pub chat_keys: HashMap<String, ChatKey>,
    #[serde(default)]
    pub group_join_keys: HashMap<String, GroupJoinKey>,
    pub settings: Settings,
    #[serde(default)]
    pub message_sequence_counters: HashMap<String, u64>,
    #[serde(skip)]
    pub delivery_tx: DeliveryTx,
    #[serde(skip)]
    pub delivery_rx: Option<UnboundedReceiver<QueuedDelivery>>,
    #[serde(skip)]
    pub replication_tx: ReplicationTx,
    #[serde(skip)]
    pub replication_rx: Option<UnboundedReceiver<ReplicationTask>>,
    #[serde(skip)]
    pub replication_wake_tx: Option<ReplicationWakeTx>,
    #[serde(skip)]
    pub replication_wake_rx: Option<ReplicationWakeRx>,
    #[serde(skip)]
    pub replication_work_inflight: Arc<AtomicBool>,
    #[serde(skip)]
    pub replication_queue: VecDeque<ReplicationTask>,
    #[serde(skip)]
    pub broker_queues: HashMap<String, VecDeque<BrokerEnvelope>>,
    #[serde(skip)]
    pub broker_offsets: HashMap<String, u64>,
    #[serde(skip)]
    pub broker_cursors: HashMap<String, u64>,
    #[serde(skip)]
    pub delivery_dedupe: HashMap<u64, u64>,
    #[serde(skip)]
    pub subscriber_events: VecDeque<SubscriberDeliveryEvent>,
    #[serde(skip)]
    pub replication_metrics: ReplicationMetrics,
    #[serde(skip)]
    pub pending_deliveries: Arc<Mutex<HashMap<String, Vec<ChatMessage>>>>,
    #[serde(skip)]
    pub ws_connections: HashMap<u32, String>,
    #[serde(skip)]
    pub browser_connections: HashMap<String, u32>,
    #[serde(skip)]
    pub last_heartbeat: HashMap<u32, u64>,
    #[serde(skip)]
    pub active_connections: HashSet<u32>,
    #[serde(skip)]
    pub node_profiles: HashMap<String, UserProfile>,
    #[serde(default)]
    pub groups: HashMap<GroupId, Group>,
    #[serde(default)]
    pub group_unread: HashMap<GroupId, u32>,
    #[serde(default)]
    pub group_notify: HashMap<GroupId, bool>,
    #[serde(skip)]
    pub(crate) search_index: SearchIndex,
    #[serde(skip)]
    pub membership_rule_cache: HashMap<GroupId, Vec<MembershipRuleBox>>,
    #[serde(skip)]
    pub group_doc_managers: HashMap<GroupId, GroupCrdtManager>,
    #[serde(skip)]
    pub groups_pending_bootstrap: HashSet<GroupId>,
    #[serde(skip)]
    pub pubsub: PubSubRegistry,
}

impl Default for ChatState {
    fn default() -> Self {
        let (delivery_tx, delivery_rx) = DeliveryTx::new();
        let (replication_tx, replication_rx) = ReplicationTx::new();
        let (replication_wake_tx, replication_wake_rx) = ReplicationWakeTx::new();

        ChatState {
            profile: UserProfile::default(),
            chats: HashMap::new(),
            chat_keys: HashMap::new(),
            group_join_keys: HashMap::new(),
            settings: Settings::default(),
            message_sequence_counters: HashMap::new(),
            delivery_tx,
            // represents "still available" versus "already consumed"
            delivery_rx: Some(delivery_rx),
            replication_tx,
            replication_rx: Some(replication_rx),
            replication_wake_tx: Some(replication_wake_tx),
            replication_wake_rx: Some(replication_wake_rx),
            replication_work_inflight: Arc::new(AtomicBool::new(false)),
            replication_queue: VecDeque::new(),
            broker_queues: HashMap::new(),
            broker_offsets: HashMap::new(),
            broker_cursors: HashMap::new(),
            delivery_dedupe: HashMap::new(),
            subscriber_events: VecDeque::new(),
            replication_metrics: ReplicationMetrics::default(),
            pending_deliveries: Arc::new(Mutex::new(HashMap::new())),
            ws_connections: HashMap::new(),
            browser_connections: HashMap::new(),
            last_heartbeat: HashMap::new(),
            active_connections: HashSet::new(),
            node_profiles: HashMap::new(),
            groups: HashMap::new(),
            group_unread: HashMap::new(),
            group_notify: HashMap::new(),
            search_index: SearchIndex::default(),
            membership_rule_cache: HashMap::new(),
            group_doc_managers: HashMap::new(),
            groups_pending_bootstrap: HashSet::new(),
            pubsub: PubSubRegistry::new(),
        }
    }
}

impl DeliveryTx {
    pub fn new() -> (Self, UnboundedReceiver<QueuedDelivery>) {
        let (sender, receiver) = mpsc::unbounded();
        (DeliveryTx { sender }, receiver)
    }

    pub fn unbounded_send(
        &self,
        delivery: QueuedDelivery,
    ) -> Result<(), mpsc::TrySendError<QueuedDelivery>> {
        self.sender.unbounded_send(delivery)
    }
}

impl<'de> Deserialize<'de> for ChatState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ChatStateSerdeV2 {
            profile: UserProfile,
            chats: HashMap<String, Chat>,
            chat_keys: HashMap<String, ChatKey>,
            #[serde(default)]
            group_join_keys: HashMap<String, GroupJoinKey>,
            settings: Settings,
            #[serde(default)]
            message_sequence_counters: HashMap<String, u64>,
            #[serde(default)]
            groups: HashMap<GroupId, Group>,
            #[serde(default)]
            group_unread: HashMap<GroupId, u32>,
            #[serde(default)]
            group_notify: HashMap<GroupId, bool>,
            #[serde(default)]
            node_profiles: HashMap<String, UserProfile>,
        }

        #[derive(Deserialize)]
        struct ChatStateSerdeV2Legacy {
            profile: UserProfile,
            chats: HashMap<String, Chat>,
            chat_keys: HashMap<String, ChatKey>,
            settings: Settings,
            #[serde(default)]
            message_sequence_counters: HashMap<String, u64>,
            #[serde(default)]
            groups: HashMap<GroupId, Group>,
            #[serde(default)]
            group_unread: HashMap<GroupId, u32>,
            #[serde(default)]
            group_notify: HashMap<GroupId, bool>,
            #[serde(default)]
            node_profiles: HashMap<String, UserProfile>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ChatStateSerdeV1 {
            profile: UserProfile,
            chats: HashMap<String, Chat>,
            chat_keys: HashMap<String, ChatKey>,
            settings: Settings,
            #[serde(default)]
            delivery_queue: HashMap<String, Vec<ChatMessage>>,
            #[serde(default)]
            online_nodes: HashSet<String>,
            #[serde(default)]
            ws_connections: HashMap<u32, String>,
            #[serde(default)]
            browser_connections: HashMap<String, u32>,
            #[serde(default)]
            last_heartbeat: HashMap<u32, u64>,
            #[serde(default)]
            active_connections: HashSet<u32>,
            #[serde(default)]
            node_profiles: HashMap<String, UserProfile>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ChatStateCompat {
            V2(ChatStateSerdeV2),
            V2Legacy(ChatStateSerdeV2Legacy),
            V1(ChatStateSerdeV1),
        }

        let (
            profile,
            chats,
            chat_keys,
            group_join_keys,
            settings,
            message_sequence_counters,
            groups,
            group_unread,
            group_notify,
            node_profiles,
        ) =
            match ChatStateCompat::deserialize(deserializer)? {
                ChatStateCompat::V2(data) => (
                    data.profile,
                    data.chats,
                    data.chat_keys,
                    data.group_join_keys,
                    data.settings,
                    data.message_sequence_counters,
                    data.groups,
                    data.group_unread,
                    data.group_notify,
                    data.node_profiles,
                ),
                ChatStateCompat::V2Legacy(data) => (
                    data.profile,
                    data.chats,
                    data.chat_keys,
                    HashMap::new(),
                    data.settings,
                    data.message_sequence_counters,
                    data.groups,
                    data.group_unread,
                    data.group_notify,
                    data.node_profiles,
                ),
                ChatStateCompat::V1(data) => (
                    data.profile,
                    data.chats,
                    data.chat_keys,
                    HashMap::new(),
                    data.settings,
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                    HashMap::new(),
                    data.node_profiles,
                ),
            };
        let (delivery_tx, delivery_rx) = DeliveryTx::new();
        let (replication_tx, replication_rx) = ReplicationTx::new();
        let (replication_wake_tx, replication_wake_rx) = ReplicationWakeTx::new();

        let mut state = ChatState {
            profile,
            chats,
            chat_keys,
            group_join_keys,
            settings,
            message_sequence_counters,
            delivery_tx,
            delivery_rx: Some(delivery_rx),
            replication_tx,
            replication_rx: Some(replication_rx),
            replication_wake_tx: Some(replication_wake_tx),
            replication_wake_rx: Some(replication_wake_rx),
            replication_work_inflight: Arc::new(AtomicBool::new(false)),
            replication_queue: VecDeque::new(),
            broker_queues: HashMap::new(),
            broker_offsets: HashMap::new(),
            broker_cursors: HashMap::new(),
            delivery_dedupe: HashMap::new(),
            subscriber_events: VecDeque::new(),
            replication_metrics: ReplicationMetrics::default(),
            pending_deliveries: Arc::new(Mutex::new(HashMap::new())),
            ws_connections: HashMap::new(),
            browser_connections: HashMap::new(),
            last_heartbeat: HashMap::new(),
            active_connections: HashSet::new(),
            node_profiles,
            groups,
            group_unread,
            group_notify,
            search_index: SearchIndex::default(),
            membership_rule_cache: HashMap::new(),
            group_doc_managers: HashMap::new(),
            groups_pending_bootstrap: HashSet::new(),
            pubsub: PubSubRegistry::new(),
        };

        if !cfg!(test) {
            if let Err(err) = state.rebuild_group_doc_managers() {
                crate::log_debug!(
                    "Failed to rebuild group CRDT managers from snapshot: {:?}",
                    err
                );
            }
        }

        Ok(state)
    }
}

impl ChatState {
    pub(crate) fn local_group_acl_ready(&self, group_id: &GroupId) -> bool {
        let Some(whitelist) = self.pubsub.whitelist(group_id) else {
            return false;
        };
        let Some(routing) = self.pubsub.routing(group_id) else {
            return false;
        };
        let node = BrokerNodeId::new(our().node.clone());
        let now = SystemTime::now();
        let hub_ok = if routing.hub_topic.is_empty() {
            false
        } else {
            let topic = BrokerTopicId::new(routing.hub_topic.clone());
            whitelist.subscribe_scope(&node, &topic, now).is_some()
                || whitelist.publish_scope(&node, &topic, now).is_some()
        };
        let sub_ok = if routing.subscriber_topic.is_empty() {
            false
        } else {
            let topic = BrokerTopicId::new(routing.subscriber_topic.clone());
            whitelist.subscribe_scope(&node, &topic, now).is_some()
        };
        hub_ok || sub_ok
    }

    #[cfg(feature = "test-helpers")]
    pub fn now_secs() -> u64 {
        Self::now_secs_inner()
    }

    #[cfg(not(feature = "test-helpers"))]
    pub(crate) fn now_secs() -> u64 {
        Self::now_secs_inner()
    }

    fn now_secs_inner() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn rebuild_group_doc_managers(&mut self) -> Result<(), CommitteeError> {
        self.ensure_routing_defaults_for_all();
        self.group_doc_managers.clear();
        self.groups_pending_bootstrap.clear();
        for (group_id, group) in &self.groups {
            // Skip groups where we've been removed - no need to bootstrap those
            let is_removed = group
                .members
                .get(&our().node)
                .map(|m| m.status == MembershipStatus::Removed)
                .unwrap_or(false);
            if is_removed {
                continue;
            }

            if self.should_seed_group_doc(group) {
                let manager = GroupCrdtManager::from_group(group_id, group)?;
                self.group_doc_managers.insert(group_id.clone(), manager);
            } else {
                self.groups_pending_bootstrap.insert(group_id.clone());
            }
        }
        self.pubsub.rebuild_all(&self.groups);
        Ok(())
    }

    pub fn rebuild_pubsub_for_group(&mut self, group_id: &GroupId) {
        if let Some(group) = self.groups.get(group_id) {
            self.pubsub.rebuild_group(group_id, group);
        } else {
            self.pubsub.remove_group(group_id);
        }
    }

    pub fn rebuild_search_index(&mut self) {
        let our_node = our().node.clone();
        self.search_index
            .rebuild(&self.chats, &self.groups, &our_node);
    }

    pub fn rebuild_chat_search(&mut self, chat_id: &str) {
        if let Some(chat) = self.chats.get(chat_id) {
            self.search_index.rebuild_chat(chat_id, chat);
        } else {
            self.search_index.remove_chat(chat_id);
        }
    }

    pub fn rebuild_group_search(&mut self, group_id: &GroupId) {
        let our_node = our().node.clone();
        if let Some(group) = self.groups.get(group_id) {
            self.search_index
                .rebuild_group(group_id, group, &our_node);
        } else {
            self.search_index.remove_group(group_id);
        }
    }

    fn ensure_routing_defaults_for_all(&mut self) {
        for (group_id, group) in self.groups.iter_mut() {
            if group.routing.hub_topic.is_empty() || group.routing.subscriber_topic.is_empty() {
                group.routing = GroupRoutingConfig::for_group(group_id);
            }
        }
    }

    pub(crate) fn require_hub_access(
        &self,
        group_id: &GroupId,
        node_id: &NodeId,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get(group_id)
            .ok_or_else(|| "group not found".to_string())?;
        let Some(whitelist) = self.pubsub.whitelist(group_id) else {
            return Err("whitelist missing".into());
        };
        if group.routing.hub_topic.is_empty() {
            return Err("hub topic unavailable".into());
        }
        let topic = BrokerTopicId::new(group.routing.hub_topic.clone());
        let node = BrokerNodeId::new(node_id.clone());
        if whitelist
            .publish_scope(&node, &topic, SystemTime::now())
            .is_some()
        {
            Ok(())
        } else {
            Err(format!(
                "node {} lacks publish access for group {}",
                node_id, group_id
            ))
        }
    }

    pub(crate) fn require_hub_subscription(
        &self,
        group_id: &GroupId,
        node_id: &NodeId,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get(group_id)
            .ok_or_else(|| "group not found".to_string())?;
        let Some(whitelist) = self.pubsub.whitelist(group_id) else {
            return Err("whitelist missing".into());
        };
        if group.routing.hub_topic.is_empty() {
            return Err("hub topic unavailable".into());
        }
        let topic = BrokerTopicId::new(group.routing.hub_topic.clone());
        let node = BrokerNodeId::new(node_id.clone());
        if whitelist
            .subscribe_scope(&node, &topic, SystemTime::now())
            .is_some()
        {
            Ok(())
        } else {
            Err(format!(
                "node {} lacks hub subscription for group {}",
                node_id, group_id
            ))
        }
    }

    pub(crate) fn require_subscriber_access(
        &self,
        group_id: &GroupId,
        node_id: &NodeId,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get(group_id)
            .ok_or_else(|| "group not found".to_string())?;
        let Some(whitelist) = self.pubsub.whitelist(group_id) else {
            return Err("whitelist missing".into());
        };
        if group.routing.subscriber_topic.is_empty() {
            return Err("subscriber topic unavailable".into());
        }
        let topic = BrokerTopicId::new(group.routing.subscriber_topic.clone());
        let node = BrokerNodeId::new(node_id.clone());
        if whitelist
            .subscribe_scope(&node, &topic, SystemTime::now())
            .is_some()
        {
            Ok(())
        } else {
            Err(format!(
                "node {} lacks subscribe access for group {}",
                node_id, group_id
            ))
        }
    }

    pub(crate) fn require_group_permission(
        &self,
        group_id: &GroupId,
        node_id: &NodeId,
        permission: u64,
    ) -> Result<(), String> {
        let group = self
            .groups
            .get(group_id)
            .ok_or_else(|| "group not found".to_string())?;
        let member = group
            .members
            .get(node_id)
            .ok_or_else(|| format!("{} is not a member of {}", node_id, group_id))?;
        if member.status != MembershipStatus::Active {
            return Err(format!(
                "member {} is not active in group {}",
                node_id, group_id
            ));
        }
        let role = group.roles.get(&member.role_id).ok_or_else(|| {
            format!(
                "role {} for {} missing in group {}",
                member.role_id, node_id, group_id
            )
        })?;
        if role.permissions.contains(permission) {
            Ok(())
        } else {
            Err(format!(
                "node {} lacks required permission in group {}",
                node_id, group_id
            ))
        }
    }

    pub fn publish_group_delta(&mut self, group_id: &GroupId, update_payload: &str) {
        let local_node = our().node.clone();
        if let Err(err) = self.require_hub_access(group_id, &local_node) {
            crate::log_debug!(
                "[CRDT][{}] skip publish: node {} lacks hub access ({})",
                group_id, local_node, err
            );
            self.replication_metrics.acl_skips =
                self.replication_metrics.acl_skips.saturating_add(1);
            return;
        }
        let (hub_topic, sub_topic) = self
            .groups
            .get(group_id)
            .map(|g| {
                (
                    g.routing.hub_topic.clone(),
                    g.routing.subscriber_topic.clone(),
                )
            })
            .unwrap_or_default();
        let acl_version = self.pubsub.whitelist(group_id).map(|w| w.version());
        if !hub_topic.is_empty() {
            self.publish_broker_message(
                &hub_topic,
                update_payload,
                acl_version,
                ReplicationKind::PushDelta,
            );
        }
        if !sub_topic.is_empty() {
            self.publish_broker_message(
                &sub_topic,
                update_payload,
                acl_version,
                ReplicationKind::PushDelta,
            );
        }
    }

    pub fn groups(&self) -> &HashMap<GroupId, Group> {
        &self.groups
    }

    pub fn groups_mut(&mut self) -> &mut HashMap<GroupId, Group> {
        &mut self.groups
    }

    pub fn group(&self, group_id: &GroupId) -> Option<&Group> {
        self.groups.get(group_id)
    }

    pub fn group_mut(&mut self, group_id: &GroupId) -> Option<&mut Group> {
        self.groups.get_mut(group_id)
    }

    pub fn upsert_group(&mut self, group_id: GroupId, group: Group) -> Option<Group> {
        self.groups.insert(group_id, group)
    }

    pub fn remove_group(&mut self, group_id: &GroupId) -> Option<Group> {
        let removed = self.groups.remove(group_id);
        self.group_doc_managers.remove(group_id);
        self.groups_pending_bootstrap.remove(group_id);
        self.membership_rule_cache.remove(group_id);
        self.pubsub.remove_group(group_id);
        removed
    }

    fn should_seed_group_doc(&self, group: &Group) -> bool {
        group
            .metadata
            .as_ref()
            .map(|meta| meta.creator_id == our().node)
            .unwrap_or(false)
    }

    pub fn group_needs_bootstrap(&self, group_id: &GroupId) -> bool {
        let needs =
            self.groups_pending_bootstrap.contains(group_id) || !self.groups.contains_key(group_id);
        if needs {
            crate::log_debug!(
                "[BOOT] group_needs_bootstrap group_id={} pending_set_contains={} has_group={}",
                group_id,
                self.groups_pending_bootstrap.contains(group_id),
                self.groups.contains_key(group_id)
            );
        }
        needs
    }

    pub(crate) fn refresh_bootstrap_flags(&mut self) {
        let ready: Vec<GroupId> = self
            .groups_pending_bootstrap
            .iter()
            .filter(|gid| {
                let has_local_membership = self
                    .groups
                    .get(*gid)
                    .and_then(|g| g.members.get(&our().node))
                    .map(|m| m.status == MembershipStatus::Active)
                    .unwrap_or(false);
                let acl_ready = self.local_group_acl_ready(gid);
                has_local_membership || acl_ready
            })
            .cloned()
            .collect();
        for gid in ready {
            crate::log_debug!(
                "[BOOT] clearing pending_bootstrap for {} (acl_ready={} local_member_active={})",
                gid,
                self.local_group_acl_ready(&gid),
                self.groups
                    .get(&gid)
                    .and_then(|g| g.members.get(&our().node))
                    .map(|m| m.status == MembershipStatus::Active)
                    .unwrap_or(false)
            );
            self.groups_pending_bootstrap.remove(&gid);
        }
    }

    pub fn mark_group_bootstrapped(&mut self, group_id: &GroupId) {
        crate::log_debug!(
            "[BOOT] mark_group_bootstrapped group_id={} pending_before={}",
            group_id,
            self.groups_pending_bootstrap.contains(group_id)
        );
        self.groups_pending_bootstrap.remove(group_id);
    }

    pub fn ensure_group_doc_manager(
        &mut self,
        group_id: &GroupId,
    ) -> Result<&mut GroupCrdtManager, CommitteeError> {
        if !self.group_doc_managers.contains_key(group_id) {
            // If we don't have the group yet, create an empty doc so the first
            // incoming snapshot can populate it without being merged with a
            // default-initialised state.
            let manager = if let Some(group) = self.groups.get(group_id) {
                GroupCrdtManager::from_group(group_id, group)?
            } else {
                GroupCrdtManager::from_empty(group_id)?
            };
            self.group_doc_managers.insert(group_id.clone(), manager);
        }

        Ok(self
            .group_doc_managers
            .get_mut(group_id)
            .expect("group manager initialised"))
    }

    pub fn commit_group_crdt(&mut self, group_id: &GroupId) -> Result<(), CommitteeError> {
        if self.group_needs_bootstrap(group_id) {
            return Err(CommitteeError::Observer(format!(
                "group {} requires bootstrap before CRDT commit",
                group_id
            )));
        }

        let group = self.groups.get(group_id).ok_or_else(|| {
            CommitteeError::Observer(format!("missing group {} for CRDT commit", group_id))
        })?;
        let snapshot: crate::GroupDocState = (group_id, group).into();
        let manager = self.ensure_group_doc_manager(group_id)?;
        manager.refresh_with_snapshot(snapshot)?;
        let state_vector = {
            let doc = manager.doc();
            let state_vector = doc.state_vector();
            crate::log_crdt_event(doc.id(), "commit_group_crdt", &state_vector, None);
            state_vector
        };
        manager.set_last_state_vector(state_vector.clone());
        self.update_local_hub_sync_state(group_id, &state_vector);
        // Enqueue per-peer fanout BEFORE rebuilding whitelist, so that members
        // who are being removed can still push their final update while they
        // still have permissions in the old whitelist.
        self.enqueue_replication_pushes(group_id, state_vector.encode_v1());
        // Rebuild whitelist AFTER enqueueing replication, so membership changes
        // can propagate before the member loses publish permissions.
        let group = self.groups.get(group_id).ok_or_else(|| {
            CommitteeError::Observer(format!("missing group {} for whitelist rebuild", group_id))
        })?;
        self.pubsub.rebuild_group(group_id, group);
        // Notify browser clients so they can refresh group state without polling.
        self.broadcast_ws_message(&WsServerMessage::GroupUpdate {
            group_id: group_id.clone(),
        });
        Ok(())
    }

    pub fn commit_group_crdt_or_log(&mut self, group_id: &GroupId, context: &str) {
        if let Err(err) = self.commit_group_crdt(group_id) {
            crate::log_debug!(
                "Failed to commit group CRDT state (group={} context={}): {:?}",
                group_id, context, err
            );
        }
    }

    pub fn create_group_state(
        &mut self,
        mut req: CreateGroupReq,
    ) -> Result<CreateGroupRes, String> {
        let group_id = req.group_id.take().unwrap_or_else(generate_group_id);
        if self.groups.contains_key(&group_id) {
            return Err("Group already exists".to_string());
        }

        let now = current_timestamp();
        let creator = our().node.clone();
        let mut counters = crate::crdt::GroupCounters::default();
        let root_thread_id = counters.next_thread_id(&group_id);

        let default_role_id = format!("{group_id}:member");
        let owner_role_id = format!("{group_id}:owner");
        let visibility = req.visibility.unwrap_or(GroupVisibility::Private);

        let metadata = GroupMetadata::new(
            req.name,
            req.description,
            req.avatar,
            creator.clone(),
            now,
            now,
            visibility,
            default_role_id.clone(),
            root_thread_id.clone(),
        );

        let mut group = Group::new(metadata);
        group.routing = GroupRoutingConfig::for_group(&group_id);
        group.counters = counters;

        let owner_role = Role::new(
            owner_role_id.clone(),
            "Owner",
            crate::crdt::GroupPermissions::all(),
            GroupTier::Hub,
        );
        let mut member_permissions = crate::crdt::GroupPermissions::empty();
        member_permissions.insert(crate::crdt::GroupPermissions::SEND_MESSAGES);
        member_permissions.insert(crate::crdt::GroupPermissions::CREATE_THREADS);
        let member_role = Role::new(
            default_role_id.clone(),
            req.default_role_label
                .unwrap_or_else(|| "Member".to_string()),
            member_permissions,
            GroupTier::Subscriber,
        );

        group.roles.insert(owner_role_id.clone(), owner_role);
        group.roles.insert(default_role_id.clone(), member_role);

        group.members.insert(
            creator.clone(),
            GroupMember::new(
                creator.clone(),
                owner_role_id,
                MembershipStatus::Active,
                now,
            ),
        );
        group.hubs.active.insert(creator.clone());
        group.hubs.upsert_sync(
            creator.clone(),
            HubSyncState {
                last_seen_ts: now,
                ..HubSyncState::default()
            },
        );
        group.subscribers.entries.insert(
            creator.clone(),
            SubscriberSyncState {
                last_state_vector: None,
                last_snapshot_digest: None,
                last_seen_ts: now,
            },
        );
        group.delivery.hub_cursors.insert(
            creator.clone(),
            crate::crdt::DeliveryCursor {
                queue_id: group.routing.hub_topic.clone(),
                last_offset: 0,
                updated_at: now,
            },
        );
        group.delivery.subscriber_cursors.insert(
            creator.clone(),
            crate::crdt::DeliveryCursor {
                queue_id: group.routing.subscriber_topic.clone(),
                last_offset: 0,
                updated_at: now,
            },
        );

        group.membership_rules = ensure_membership_rules(req.membership_rules, &creator);

        let mut root_thread = Thread::new(
            root_thread_id.clone(),
            group_id.clone(),
            0,
            ThreadParentRef::Root(group_id.clone()),
            now,
            creator.clone(),
        );
        root_thread.title = req.root_thread_title;
        root_thread.summary.last_activity = now;
        root_thread.summary.last_sender = Some(creator);
        group.threads.insert(root_thread_id, root_thread);

        self.groups.insert(group_id.clone(), group);
        self.mark_group_bootstrapped(&group_id);
        self.commit_group_crdt_or_log(&group_id, "create_group");
        self.rebuild_group_search(&group_id);

        Ok(CreateGroupRes { group_id })
    }

    pub fn list_groups_state(&self) -> ListGroupsRes {
        let caller = our().node;
        let groups = self
            .groups
            .iter()
            .filter(|(_group_id, group)| {
                // Only include groups where the caller is an active member
                group
                    .members
                    .get(&caller)
                    .map(|m| m.status == MembershipStatus::Active)
                    .unwrap_or(false)
            })
            .map(|(group_id, group)| GroupSummary {
                group_id: group_id.clone(),
                metadata: group.metadata.clone(),
                member_count: group.members.len(),
                thread_count: group.threads.len(),
                unread_count: self.group_unread.get(group_id).copied().unwrap_or(0),
                notify: self.group_notify.get(group_id).copied().unwrap_or(true),
            })
            .collect();
        ListGroupsRes { groups }
    }

    pub fn get_group_state(&self, req: GetGroupReq) -> GetGroupRes {
        let group = self.groups.get(&req.group_id).cloned();
        if let Some(ref g) = group {
            for msg in g.messages.values() {
                if !msg.reactions.is_empty() {
                    crate::log_debug!(
                        "[GET_GROUP] msg_id={} has {} reactions: {:?}",
                        msg.message_id,
                        msg.reactions.len(),
                        msg.reactions.iter().map(|r| &r.emoji).collect::<Vec<_>>()
                    );
                }
            }
        }
        GetGroupRes { group }
    }

    pub fn create_group_thread_state(
        &mut self,
        mut req: CreateGroupThreadReq,
    ) -> Result<CreateGroupThreadRes, String> {
        self.require_group_permission(
            &req.group_id,
            &our().node,
            crate::crdt::GroupPermissions::CREATE_THREADS,
        )
        .map_err(|err| format!("cannot create thread: {}", err))?;
        self.require_subscriber_access(&req.group_id, &our().node)
            .map_err(|err| format!("cannot create thread: {}", err))?;

        let thread_id = self.next_group_thread_id(&req.group_id)?;
        let now = current_timestamp();
        let creator = our().node.clone();

        {
            let group = self
                .groups
                .get_mut(&req.group_id)
                .ok_or_else(|| "Group not found".to_string())?;

            let root_thread_id = group_root_thread_id(group)
                .ok_or_else(|| "Group missing root thread".to_string())?;

            let (parent_ref, depth, parent_child) =
                if let Some(parent_id) = req.parent_thread_id.take() {
                    let parent = group
                        .threads
                        .get(&parent_id)
                        .ok_or_else(|| "Parent thread not found".to_string())?;
                    (
                        ThreadParentRef::Thread(parent_id.clone()),
                        parent.depth + 1,
                        Some(parent_id.clone()),
                    )
                } else {
                    (
                        ThreadParentRef::Root(root_thread_id.clone()),
                        0,
                        Some(root_thread_id.clone()),
                    )
                };

            // Validate and attach root message if provided.
            let root_message_id = if let Some(root_id) = req.root_message_id.take() {
                let msg = group
                    .messages
                    .get(&root_id)
                    .ok_or_else(|| "Root message not found".to_string())?;
                // Ensure the message belongs to the chosen parent thread (or root).
                if let Some(parent_id) = &parent_child {
                    if &msg.thread_id != parent_id {
                        return Err("Root message does not belong to parent thread".to_string());
                    }
                }
                Some(root_id)
            } else {
                None
            };

            let mut thread = Thread::new(
                thread_id.clone(),
                req.group_id.clone(),
                depth,
                parent_ref,
                now,
                creator,
            );
            thread.title = req.title.take();
            thread.summary.last_activity = now;
            thread.root_message_id = root_message_id;

            group.threads.insert(thread_id.clone(), thread);
            if let Some(parent_id) = parent_child {
                if let Some(parent) = group.threads.get_mut(&parent_id) {
                    parent.child_threads.push(thread_id.clone());
                }
            }
            if let Some(meta) = group.metadata.as_mut() {
                // Ensure updated_at advances even if called within the same second.
                let mut ts = now;
                if meta.updated_at >= ts {
                    ts = meta.updated_at.saturating_add(1);
                }
                meta.updated_at = ts;
            }
        }
        self.commit_group_crdt_or_log(&req.group_id, "create_group_thread");
        Ok(CreateGroupThreadRes { thread_id })
    }
}
