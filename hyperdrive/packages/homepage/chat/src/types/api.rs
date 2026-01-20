use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::crdt::{
    AttachmentDescriptor, DeliveryCursor, Group, GroupId, GroupMetadata, GroupRoutingConfig,
    GroupVisibility, MembershipDecision, MembershipRuleConfig, MembershipRuleError, MessageId,
    MessageMeta, NodeId, ThreadId,
};

use super::{
    default_group_message_type, FileInfo, MessageType, ReplicationMetrics,
    SubscriberDeliveryEvent,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateChatReq {
    pub counterparty: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetChatReq {
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetMessagesReq {
    pub chat_id: String,
    pub before_timestamp: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetSyncHashReq {
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncHashInfo {
    pub chat_id: String,
    pub message_count: u32,
    pub last_message_id: Option<String>,
    pub last_message_timestamp: Option<u64>,
    pub hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteChatReq {
    pub chat_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateChatSettingsReq {
    pub chat_id: String,
    pub notify: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendMessageReq {
    pub chat_id: String,
    pub content: String,
    pub reply_to: Option<String>,
    pub file_info: Option<FileInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditMessageReq {
    pub chat_id: String,
    pub message_id: String,
    pub new_content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteMessageReq {
    pub chat_id: String,
    pub message_id: String,
    pub delete_for_both: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddReactionReq {
    pub chat_id: String,
    pub message_id: String,
    pub emoji: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveReactionReq {
    pub chat_id: String,
    pub message_id: String,
    pub emoji: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ForwardMessageReq {
    pub from_chat_id: String,
    pub message_id: String,
    pub to_chat_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateChatLinkReq {
    pub chat_id: String,
    pub single_use: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RevokeChatKeyReq {
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadFileReq {
    pub chat_id: String,
    pub filename: String,
    pub mime_type: String,
    pub data: String,
    pub reply_to: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadGroupFileReq {
    pub group_id: GroupId,
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    #[serde(default)]
    pub reply_to: Option<MessageId>,
    pub filename: String,
    pub mime_type: String,
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadFileReq {
    pub chat_id: String,
    pub file_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadGroupFileReq {
    pub group_id: GroupId,
    pub attachment_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchGroupFileReq {
    pub group_id: GroupId,
    pub attachment_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadProfilePictureReq {
    pub mime_type: String,
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendVoiceNoteReq {
    pub chat_id: String,
    pub audio_data: String,
    pub duration: u32,
    pub reply_to: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendGroupVoiceNoteReq {
    pub group_id: GroupId,
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    #[serde(default)]
    pub reply_to: Option<MessageId>,
    pub audio_data: String,
    pub duration: u32,
    pub mime_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchChatsReq {
    pub query: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchScope {
    Chats,
    Groups,
    Messages,
    All,
}

impl Default for SearchScope {
    fn default() -> Self {
        SearchScope::All
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchResultKind {
    ChatSummary,
    ChatMessage,
    GroupSummary,
    GroupMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchIndexReq {
    pub query: String,
    #[serde(default)]
    pub scope: SearchScope,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchResultItem {
    pub kind: SearchResultKind,
    #[serde(default)]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub group_id: Option<GroupId>,
    #[serde(default)]
    pub message_id: Option<MessageId>,
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    pub title: String,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchIndexRes {
    pub results: Vec<SearchResultItem>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupReq {
    #[serde(default)]
    pub group_id: Option<GroupId>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub visibility: Option<GroupVisibility>,
    #[serde(default)]
    pub default_role_label: Option<String>,
    #[serde(default)]
    pub membership_rules: Vec<MembershipRuleConfig>,
    #[serde(default)]
    pub root_thread_title: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupRes {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupThreadReq {
    pub group_id: GroupId,
    #[serde(default)]
    pub parent_thread_id: Option<ThreadId>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub root_message_id: Option<MessageId>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupThreadRes {
    pub thread_id: ThreadId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendGroupMessageReq {
    pub group_id: GroupId,
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
    pub content: String,
    #[serde(default = "default_group_message_type")]
    pub message_type: MessageType,
    #[serde(default)]
    pub reply_to: Option<MessageId>,
    #[serde(default)]
    pub attachments: Vec<AttachmentDescriptor>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendGroupMessageRes {
    pub message: MessageMeta,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditGroupMessageReq {
    pub group_id: GroupId,
    pub message_id: MessageId,
    pub new_content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteGroupMessageReq {
    pub group_id: GroupId,
    pub message_id: MessageId,
    #[serde(default)]
    pub delete_for_both: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddGroupReactionReq {
    pub group_id: GroupId,
    pub message_id: MessageId,
    pub emoji: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveGroupReactionReq {
    pub group_id: GroupId,
    pub message_id: MessageId,
    pub emoji: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetGroupReq {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetGroupRes {
    pub group: Option<Group>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupSummary {
    pub group_id: GroupId,
    pub metadata: Option<GroupMetadata>,
    pub member_count: usize,
    pub thread_count: usize,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default = "default_true")]
    pub notify: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateGroupSettingsReq {
    pub group_id: GroupId,
    pub notify: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListGroupsRes {
    pub groups: Vec<GroupSummary>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupJoinLinkReq {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateGroupJoinLinkRes {
    pub link: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JoinGroupLinkReq {
    pub host: NodeId,
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JoinGroupLinkRemoteReq {
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JoinGroupLinkRes {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InviteGroupMemberReq {
    pub group_id: GroupId,
    pub candidate: NodeId,
    pub role_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApproveGroupMembershipReq {
    pub group_id: GroupId,
    pub proposal_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveGroupMemberReq {
    pub group_id: GroupId,
    pub member: NodeId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MembershipDecisionRes {
    pub decision: MembershipDecision,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtStateVectorRes {
    pub state_vector: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtGroupStateVectorReq {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtGroupUpdateReq {
    pub group_id: GroupId,
    #[serde(default)]
    pub state_vector: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtUpdateRes {
    pub doc_id: String,
    pub update_payload: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtGroupApplyReq {
    pub group_id: GroupId,
    pub update_payload: String,
    #[serde(default)]
    pub acl_version: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtGroupSnapshotReq {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CrdtApplyRes {
    pub applied: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdminReplicationStateReq {
    #[serde(default)]
    pub group_id: Option<GroupId>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupReplicationState {
    pub group_id: GroupId,
    pub pending_bootstrap: bool,
    pub routing: GroupRoutingConfig,
    pub hubs: Vec<NodeId>,
    pub subscribers: Vec<NodeId>,
    pub hub_cursors: HashMap<NodeId, DeliveryCursor>,
    pub subscriber_cursors: HashMap<NodeId, DeliveryCursor>,
    #[serde(default)]
    pub whitelist_version: Option<u64>,
    #[serde(default)]
    pub subscriber_lag_secs: Option<u64>,
    #[serde(default)]
    pub hub_lag_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdminReplicationStateRes {
    pub metrics: ReplicationMetrics,
    pub groups: Vec<GroupReplicationState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdminWhitelistReq {
    pub group_id: GroupId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WhitelistEntryDebug {
    pub node: String,
    pub publish: Vec<String>,
    pub subscribe: Vec<String>,
    pub audiences: Vec<String>,
    pub features: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdminWhitelistRes {
    pub group_id: GroupId,
    pub version: u64,
    pub entries: Vec<WhitelistEntryDebug>,
}

/// Request to immediately push a snapshot to a specific peer (bypassing the debounced queue).
/// Used when a member is invited to get them bootstrapped immediately.
#[derive(Serialize, Deserialize, Debug)]
pub struct PushSnapshotToPeerReq {
    pub group_id: GroupId,
    pub peer: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubscriberEventsReq {
    #[serde(default)]
    pub take: Option<usize>,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubscriberEventsRes {
    pub events: Vec<SubscriberDeliveryEvent>,
}

#[derive(Serialize, Deserialize, Clone, Debug, process_macros::SerdeJsonInto)]
pub enum HomepageRequest {
    GetPushSubscription,
}

#[derive(Serialize, Deserialize, Clone, Debug, process_macros::SerdeJsonInto)]
pub enum HomepageResponse {
    PushSubscription(Option<String>),
}

#[derive(Debug)]
pub enum MembershipActionError {
    GroupNotFound(GroupId),
    MemberExists(NodeId),
    MemberNotFound(NodeId),
    ProposalExists(String),
    ProposalNotFound(String),
    RuleError(MembershipRuleError),
    PermissionDenied(String),
}

impl From<MembershipRuleError> for MembershipActionError {
    fn from(err: MembershipRuleError) -> Self {
        MembershipActionError::RuleError(err)
    }
}

impl fmt::Display for MembershipActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MembershipActionError::GroupNotFound(id) => write!(f, "group '{id}' not found"),
            MembershipActionError::MemberExists(node) => {
                write!(f, "member '{node}' already exists in group")
            }
            MembershipActionError::MemberNotFound(node) => {
                write!(f, "member '{node}' not found in group")
            }
            MembershipActionError::ProposalExists(id) => {
                write!(f, "proposal '{id}' already exists")
            }
            MembershipActionError::ProposalNotFound(id) => {
                write!(f, "proposal '{id}' not found")
            }
            MembershipActionError::RuleError(err) => write!(f, "rule error: {}", err),
            MembershipActionError::PermissionDenied(msg) => {
                write!(f, "permission denied: {}", msg)
            }
        }
    }
}

impl std::error::Error for MembershipActionError {}

// Spider Integration Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderApiKey {
    pub key: String,
    pub name: String,
    pub permissions: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderConnectResult {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderStatusInfo {
    pub connected: bool,
    pub has_api_key: bool,
    #[serde(rename = "spider_available")]
    pub spider_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderMessageContent {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub audio: Option<Vec<u8>>,
    #[serde(rename = "base-six-four-audio", default)]
    pub base_six_four_audio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderMessage {
    pub role: String,
    pub content: SpiderMessageContent,
    #[serde(rename = "tool-calls-json", default)]
    pub tool_calls_json: Option<String>,
    #[serde(rename = "tool-results-json", default)]
    pub tool_results_json: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderHistory {
    pub messages: Vec<SpiderMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiderSetHistoryReq {
    pub messages: Vec<SpiderMessage>,
}
