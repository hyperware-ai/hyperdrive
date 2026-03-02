use std::collections::{HashMap, HashSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::MessageType;

/// Canonical identifier for a replicated group.
pub type GroupId = String;

/// Deterministic identifier for a thread that belongs to a [`GroupId`].
pub type ThreadId = String;

/// Deterministic identifier for a message that belongs to a [`ThreadId`].
pub type MessageId = String;

/// Represents a node (peer) that can join a group.
pub type NodeId = String;

/// Identifier for a membership rule implementation.
pub type MembershipRuleId = String;

/// Static metadata about a group that needs to stay consistent across replicas.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupMetadata {
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub creator_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub visibility: GroupVisibility,
    pub default_role_id: String,
    pub root_thread_id: ThreadId,
}

impl GroupMetadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        description: Option<String>,
        avatar: Option<String>,
        creator_id: impl Into<String>,
        created_at: u64,
        updated_at: u64,
        visibility: GroupVisibility,
        default_role_id: impl Into<String>,
        root_thread_id: ThreadId,
    ) -> Self {
        Self {
            name: name.into(),
            description,
            avatar,
            creator_id: creator_id.into(),
            created_at,
            updated_at,
            visibility,
            default_role_id: default_role_id.into(),
            root_thread_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupVisibility {
    Private,
    #[serde(alias = "InviteOnly")]
    Public,
}

impl Default for GroupVisibility {
    fn default() -> Self {
        GroupVisibility::Private
    }
}

/// Tier communicates whether a role participates as a hub operator or a regular subscriber.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupTier {
    Hub,
    Subscriber,
}

impl Default for GroupTier {
    fn default() -> Self {
        GroupTier::Subscriber
    }
}

/// Simple bitset wrapper so we can extend permissions without changing serde layout.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GroupPermissions(pub u64);

impl Default for GroupPermissions {
    fn default() -> Self {
        GroupPermissions(0)
    }
}

impl GroupPermissions {
    pub const SEND_MESSAGES: u64 = 1 << 0;
    pub const CREATE_THREADS: u64 = 1 << 1;
    pub const INVITE_MEMBERS: u64 = 1 << 2;
    pub const MANAGE_ROLES: u64 = 1 << 3;
    pub const MANAGE_SETTINGS: u64 = 1 << 4;

    pub fn empty() -> Self {
        GroupPermissions(0)
    }

    pub fn all() -> Self {
        GroupPermissions(
            Self::SEND_MESSAGES
                | Self::CREATE_THREADS
                | Self::INVITE_MEMBERS
                | Self::MANAGE_ROLES
                | Self::MANAGE_SETTINGS,
        )
    }

    pub fn contains(self, flag: u64) -> bool {
        self.0 & flag == flag
    }

    pub fn insert(&mut self, flag: u64) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u64) {
        self.0 &= !flag;
    }
}

/// Describes a named role within a group and the permissions it grants.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    pub id: String,
    pub label: String,
    pub permissions: GroupPermissions,
    pub tier: GroupTier,
}

impl Role {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        permissions: GroupPermissions,
        tier: GroupTier,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            permissions,
            tier,
        }
    }
}

/// Tracks the membership lifecycle for a node.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MembershipStatus {
    Pending,
    Active,
    Removed,
}

impl Default for MembershipStatus {
    fn default() -> Self {
        MembershipStatus::Pending
    }
}

/// Stores the role binding and recency metadata for a specific member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupMember {
    pub node_id: NodeId,
    pub role_id: String,
    pub status: MembershipStatus,
    pub last_activity: u64,
}

impl GroupMember {
    pub fn new(
        node_id: impl Into<NodeId>,
        role_id: impl Into<String>,
        status: MembershipStatus,
        last_activity: u64,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            role_id: role_id.into(),
            status,
            last_activity,
        }
    }
}

/// Tracks hub nodes responsible for routing group traffic.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupHubSet {
    #[serde(default)]
    pub active: HashSet<NodeId>,
    #[serde(default)]
    pub pending: HashSet<NodeId>,
    #[serde(default)]
    pub sync: HashMap<NodeId, HubSyncState>,
}

impl GroupHubSet {
    pub fn new(active: HashSet<NodeId>, pending: HashSet<NodeId>) -> Self {
        Self {
            active,
            pending,
            sync: HashMap::new(),
        }
    }

    pub fn upsert_sync(&mut self, node_id: NodeId, state: HubSyncState) {
        self.sync.insert(node_id, state);
    }
}

/// Tracks hub replication progress, ensuring CRDT updates and snapshots propagate reliably.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HubSyncState {
    #[serde(default)]
    pub last_state_vector: Option<Vec<u8>>,
    #[serde(default)]
    pub last_snapshot_digest: Option<String>,
    #[serde(default)]
    pub pending_snapshot_id: Option<String>,
    pub last_seen_ts: u64,
    #[serde(default)]
    pub last_ack_seq: u64,
}

/// Captures subscriber sync info so routing policies can avoid scanning full members list.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupSubscriberSet {
    #[serde(default)]
    pub entries: HashMap<NodeId, SubscriberSyncState>,
}

impl GroupSubscriberSet {
    pub fn upsert(&mut self, node_id: NodeId, state: SubscriberSyncState) {
        self.entries.insert(node_id, state);
    }
}

/// Delivery cursor describing progress in a queue/attempt lane.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryCursor {
    pub queue_id: String,
    #[serde(default)]
    pub last_offset: u64,
    pub updated_at: u64,
}

/// Captures delivery cursor state for hubs and subscribers so queues resume cleanly.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupDeliveryState {
    #[serde(default)]
    pub hub_cursors: HashMap<NodeId, DeliveryCursor>,
    #[serde(default)]
    pub subscriber_cursors: HashMap<NodeId, DeliveryCursor>,
    #[serde(default)]
    pub attempt_seeds: HashMap<String, u64>,
}

/// Topic routing configuration for a group.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupRoutingConfig {
    pub hub_topic: String,
    pub subscriber_topic: String,
    pub snapshot_interval_secs: u64,
}

impl GroupRoutingConfig {
    pub fn new(
        hub_topic: impl Into<String>,
        subscriber_topic: impl Into<String>,
        snapshot_interval_secs: u64,
    ) -> Self {
        Self {
            hub_topic: hub_topic.into(),
            subscriber_topic: subscriber_topic.into(),
            snapshot_interval_secs,
        }
    }

    pub fn for_group(group_id: &GroupId) -> Self {
        Self::new(
            format!("chat.{group_id}.hubs"),
            format!("chat.{group_id}.subs"),
            30,
        )
    }
}

impl Default for GroupRoutingConfig {
    fn default() -> Self {
        Self::new(String::new(), String::new(), 30)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscriberSyncState {
    #[serde(default)]
    pub last_state_vector: Option<Vec<u8>>,
    pub last_snapshot_digest: Option<String>,
    pub last_seen_ts: u64,
}

/// Declarative configuration for membership rules so compiled strategies stay deterministic.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MembershipRuleConfig {
    pub rule_id: MembershipRuleId,
    #[serde(default)]
    pub params: Value,
}

impl MembershipRuleConfig {
    pub fn new(rule_id: impl Into<MembershipRuleId>, params: Value) -> Self {
        Self {
            rule_id: rule_id.into(),
            params,
        }
    }
}

/// Enumerates which membership lifecycle operation is being proposed.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MembershipActionKind {
    Invite,
    Remove,
}

impl Default for MembershipActionKind {
    fn default() -> Self {
        MembershipActionKind::Invite
    }
}

/// Captures the data points needed when evaluating a membership change proposal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipProposal {
    pub proposal_id: String,
    pub candidate: NodeId,
    pub requested_role: String,
    pub proposer: NodeId,
    #[serde(default)]
    pub action: MembershipActionKind,
    #[serde(default)]
    pub approvals: HashSet<NodeId>,
    #[serde(default)]
    pub rejections: HashSet<NodeId>,
    #[serde(default)]
    pub eligible_voters: u32,
    #[serde(default)]
    pub token_support: u128,
    #[serde(default)]
    pub token_opposition: u128,
}

impl MembershipProposal {
    pub fn approval_count(&self) -> usize {
        self.approvals.len()
    }

    pub fn rejection_count(&self) -> usize {
        self.rejections.len()
    }

    pub fn outstanding_voters(&self) -> i64 {
        let decided = self.approvals.len() + self.rejections.len();
        self.eligible_voters as i64 - decided as i64
    }
}

impl Default for MembershipProposal {
    fn default() -> Self {
        Self {
            proposal_id: String::new(),
            candidate: String::new(),
            requested_role: String::new(),
            proposer: String::new(),
            action: MembershipActionKind::Invite,
            approvals: HashSet::new(),
            rejections: HashSet::new(),
            eligible_voters: 0,
            token_support: 0,
            token_opposition: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MembershipDecisionStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipDecision {
    pub status: MembershipDecisionStatus,
    #[serde(default)]
    pub missing_signatures: Vec<NodeId>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl MembershipDecision {
    pub fn approved() -> Self {
        Self {
            status: MembershipDecisionStatus::Approved,
            missing_signatures: Vec::new(),
            reason: None,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            status: MembershipDecisionStatus::Rejected,
            missing_signatures: Vec::new(),
            reason: Some(reason.into()),
        }
    }

    pub fn pending(missing_signatures: Vec<NodeId>) -> Self {
        Self {
            status: MembershipDecisionStatus::Pending,
            missing_signatures,
            reason: None,
        }
    }
}

/// Stateless trait so different rule encodings can be evaluated uniformly.
pub trait MembershipRule: Send + Sync {
    fn rule_id(&self) -> &'static str;
    fn required_signatures(&self, proposal: &MembershipProposal) -> Vec<NodeId>;
    fn evaluate(&self, proposal: &MembershipProposal) -> MembershipDecision;
}

#[derive(Debug)]
pub enum MembershipRuleError {
    UnknownRule(String),
    InvalidParams { rule_id: String, message: String },
}

impl std::fmt::Display for MembershipRuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MembershipRuleError::UnknownRule(rule) => {
                write!(f, "unknown membership rule '{rule}'")
            }
            MembershipRuleError::InvalidParams { rule_id, message } => {
                write!(f, "invalid params for rule '{rule_id}': {message}")
            }
        }
    }
}

impl std::error::Error for MembershipRuleError {}

pub type MembershipRuleBox = Box<dyn MembershipRule>;

pub fn compile_membership_rules(
    configs: &[MembershipRuleConfig],
) -> Result<Vec<MembershipRuleBox>, MembershipRuleError> {
    configs.iter().map(build_membership_rule).collect()
}

fn build_membership_rule(
    config: &MembershipRuleConfig,
) -> Result<MembershipRuleBox, MembershipRuleError> {
    match config.rule_id.as_str() {
        "membership.rule.dictator" => {
            #[derive(Deserialize)]
            struct Params {
                dictator: NodeId,
            }
            let params: Params = parse_params(config)?;
            Ok(Box::new(DictatorRule::new(params.dictator)))
        }
        "membership.rule.multi_dictator" => {
            #[derive(Deserialize)]
            struct Params {
                dictators: Vec<NodeId>,
                #[serde(default)]
                required: Option<usize>,
            }
            let params: Params = parse_params(config)?;
            let dictators: HashSet<NodeId> = params.dictators.into_iter().collect();
            let required = params.required.unwrap_or_else(|| dictators.len().max(1));
            Ok(Box::new(MultiDictatorRule::new(dictators, required)))
        }
        "membership.rule.tally_vote" => {
            #[derive(Deserialize, Default)]
            struct Params {
                #[serde(default)]
                quorum: Option<f32>,
            }
            let params: Params = if config.params.is_null() {
                Params::default()
            } else {
                parse_params(config)?
            };
            let quorum = params.quorum.unwrap_or_else(TallyVoteRule::default_quorum);
            Ok(Box::new(TallyVoteRule::new(quorum)))
        }
        "membership.rule.token_threshold" => {
            #[derive(Deserialize, Default)]
            struct Params {
                #[serde(default)]
                min_support: Option<u128>,
                #[serde(default)]
                min_ratio: Option<f32>,
            }
            let params: Params = if config.params.is_null() {
                Params::default()
            } else {
                parse_params(config)?
            };
            Ok(Box::new(TokenThresholdRule::new(
                params.min_support.unwrap_or(0),
                params.min_ratio,
            )))
        }
        other => Err(MembershipRuleError::UnknownRule(other.to_string())),
    }
}

fn parse_params<T: DeserializeOwned>(
    config: &MembershipRuleConfig,
) -> Result<T, MembershipRuleError> {
    serde_json::from_value(config.params.clone()).map_err(|err| {
        MembershipRuleError::InvalidParams {
            rule_id: config.rule_id.clone(),
            message: err.to_string(),
        }
    })
}

/// Single-operator rule where one dictator approves or rejects every request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictatorRule {
    pub dictator: NodeId,
}

impl DictatorRule {
    pub fn new(dictator: impl Into<NodeId>) -> Self {
        Self {
            dictator: dictator.into(),
        }
    }
}

impl MembershipRule for DictatorRule {
    fn rule_id(&self) -> &'static str {
        "membership.rule.dictator"
    }

    fn required_signatures(&self, _proposal: &MembershipProposal) -> Vec<NodeId> {
        vec![self.dictator.clone()]
    }

    fn evaluate(&self, proposal: &MembershipProposal) -> MembershipDecision {
        if proposal.approvals.contains(&self.dictator) {
            MembershipDecision::approved()
        } else if proposal.rejections.contains(&self.dictator) {
            MembershipDecision::rejected("dictator rejection")
        } else {
            MembershipDecision::pending(vec![self.dictator.clone()])
        }
    }
}

/// Whitelist of trusted operators where a configurable number must approve.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultiDictatorRule {
    #[serde(default)]
    pub dictators: HashSet<NodeId>,
    pub required: usize,
}

impl MultiDictatorRule {
    pub fn new(dictators: HashSet<NodeId>, required: usize) -> Self {
        Self {
            dictators,
            required,
        }
    }

    fn approvals_from_dictators(&self, proposal: &MembershipProposal) -> usize {
        proposal
            .approvals
            .iter()
            .filter(|id| self.dictators.contains(*id))
            .count()
    }
}

impl MembershipRule for MultiDictatorRule {
    fn rule_id(&self) -> &'static str {
        "membership.rule.multi_dictator"
    }

    fn required_signatures(&self, _proposal: &MembershipProposal) -> Vec<NodeId> {
        self.dictators.iter().cloned().collect()
    }

    fn evaluate(&self, proposal: &MembershipProposal) -> MembershipDecision {
        let approvals = self.approvals_from_dictators(proposal);
        if approvals >= self.required {
            return MembershipDecision::approved();
        }

        if proposal
            .rejections
            .iter()
            .any(|node| self.dictators.contains(node))
        {
            return MembershipDecision::rejected("dictator rejection");
        }

        let mut missing: Vec<NodeId> = self
            .dictators
            .iter()
            .filter(|id| !proposal.approvals.contains(*id))
            .cloned()
            .collect();
        missing.sort();
        MembershipDecision::pending(missing)
    }
}

/// Majority/plurality vote rule with configurable quorum ratios.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TallyVoteRule {
    #[serde(default = "TallyVoteRule::default_quorum")]
    pub quorum: f32,
}

impl TallyVoteRule {
    pub fn default_quorum() -> f32 {
        0.5
    }

    pub fn new(quorum: f32) -> Self {
        Self { quorum }
    }

    fn required_votes(&self, eligible_voters: u32) -> u32 {
        if eligible_voters == 0 {
            return 0;
        }
        let quorum = self.quorum.clamp(0.0, 1.0);
        ((eligible_voters as f32 * quorum).ceil() as u32).max(1)
    }
}

impl MembershipRule for TallyVoteRule {
    fn rule_id(&self) -> &'static str {
        "membership.rule.tally_vote"
    }

    fn required_signatures(&self, _proposal: &MembershipProposal) -> Vec<NodeId> {
        Vec::new()
    }

    fn evaluate(&self, proposal: &MembershipProposal) -> MembershipDecision {
        let required = self.required_votes(proposal.eligible_voters);
        let approvals = proposal.approval_count() as u32;
        let rejections = proposal.rejection_count() as u32;

        if approvals >= required {
            return MembershipDecision::approved();
        }

        let remaining = proposal
            .eligible_voters
            .saturating_sub(approvals + rejections);
        if approvals + remaining < required {
            return MembershipDecision::rejected("quorum unreachable");
        }

        MembershipDecision::pending(Vec::new())
    }
}

/// Placeholder for future token-weighted rules.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TokenThresholdRule {
    #[serde(default)]
    pub min_support: u128,
    #[serde(default)]
    pub min_ratio: Option<f32>,
}

impl TokenThresholdRule {
    pub fn new(min_support: u128, min_ratio: Option<f32>) -> Self {
        Self {
            min_support,
            min_ratio,
        }
    }
}

impl MembershipRule for TokenThresholdRule {
    fn rule_id(&self) -> &'static str {
        "membership.rule.token_threshold"
    }

    fn required_signatures(&self, _proposal: &MembershipProposal) -> Vec<NodeId> {
        Vec::new()
    }

    fn evaluate(&self, proposal: &MembershipProposal) -> MembershipDecision {
        if proposal.token_support >= self.min_support {
            if let Some(ratio) = self.min_ratio {
                let ratio = ratio.clamp(0.0, 1.0);
                let total = proposal.token_support + proposal.token_opposition;
                if total > 0 {
                    let current_ratio = proposal.token_support as f64 / total as f64;
                    if current_ratio < ratio as f64 {
                        return MembershipDecision::pending(Vec::new());
                    }
                }
            }
            MembershipDecision::approved()
        } else if proposal.token_support + proposal.token_opposition < self.min_support {
            MembershipDecision::pending(Vec::new())
        } else {
            MembershipDecision::pending(Vec::new())
        }
    }
}

/// Identifies the parent of a thread (either the group root or another thread).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThreadParentRef {
    Root(GroupId),
    Thread(ThreadId),
}

/// Lightweight summary so clients can render thread previews.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadSummary {
    pub message_count: u64,
    pub last_message_id: Option<MessageId>,
    pub last_activity: u64,
    pub last_sender: Option<NodeId>,
}

/// Represents a thread (root or nested) within a group. IDs should be minted via [`GroupCounters`]
/// so replicas agree on ordering.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thread {
    pub id: ThreadId,
    pub group_id: GroupId,
    pub depth: u32,
    pub parent: ThreadParentRef,
    pub child_threads: Vec<ThreadId>,
    pub created_at: u64,
    pub created_by: NodeId,
    pub root_message_id: Option<MessageId>,
    pub summary: ThreadSummary,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

impl Thread {
    pub fn new(
        id: ThreadId,
        group_id: GroupId,
        depth: u32,
        parent: ThreadParentRef,
        created_at: u64,
        created_by: NodeId,
    ) -> Self {
        Self {
            id,
            group_id,
            depth,
            parent,
            child_threads: Vec::new(),
            created_at,
            created_by,
            root_message_id: None,
            summary: ThreadSummary::default(),
            title: None,
            archived: false,
        }
    }
}

/// Minimal attachment descriptor so replicas agree on included assets without full payloads.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentDescriptor {
    pub attachment_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub checksum: Option<String>,
    pub uri: Option<String>,
}

/// CRDT-friendly description of a group message without heavyweight content blobs.
/// Message IDs should be produced via [`GroupCounters`] to remain deterministic.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageMeta {
    pub message_id: MessageId,
    pub thread_id: ThreadId,
    pub group_id: GroupId,
    pub sender: NodeId,
    pub timestamp: u64,
    pub message_type: MessageType,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub reply_to: Option<MessageId>,
    #[serde(default)]
    pub reply_in_thread: Option<MessageId>,
    #[serde(default)]
    pub reactions: Vec<MessageReactionMeta>,
    #[serde(default)]
    pub attachments: Vec<AttachmentDescriptor>,
}

impl MessageMeta {
    pub fn new(
        message_id: MessageId,
        thread_id: ThreadId,
        group_id: GroupId,
        sender: NodeId,
        timestamp: u64,
        message_type: MessageType,
        body: String,
    ) -> Self {
        Self {
            message_id,
            thread_id,
            group_id,
            sender,
            timestamp,
            message_type,
            body,
            reply_to: None,
            reply_in_thread: None,
            reactions: Vec::new(),
            attachments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageReactionMeta {
    pub node_id: NodeId,
    pub emoji: String,
    pub timestamp: u64,
}

/// Bundles all replicated state for a single [`GroupId`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Group {
    #[serde(default)]
    pub metadata: Option<GroupMetadata>,
    #[serde(default)]
    pub roles: HashMap<String, Role>,
    #[serde(default)]
    pub members: HashMap<NodeId, GroupMember>,
    #[serde(default)]
    pub hubs: GroupHubSet,
    #[serde(default)]
    pub subscribers: GroupSubscriberSet,
    #[serde(default)]
    pub routing: GroupRoutingConfig,
    #[serde(default)]
    pub delivery: GroupDeliveryState,
    #[serde(default)]
    pub membership_rules: Vec<MembershipRuleConfig>,
    #[serde(default)]
    pub membership_proposals: HashMap<String, MembershipProposal>,
    #[serde(default)]
    pub threads: HashMap<ThreadId, Thread>,
    #[serde(default)]
    pub messages: HashMap<MessageId, MessageMeta>,
    #[serde(default)]
    pub counters: GroupCounters,
}

impl Group {
    pub fn new(metadata: GroupMetadata) -> Self {
        Self {
            metadata: Some(metadata),
            routing: GroupRoutingConfig::default(),
            ..Self::default()
        }
    }
}

/// Tracks monotonically increasing counters for deterministic thread/message IDs within a group.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupCounters {
    #[serde(default)]
    pub next_thread: u64,
    #[serde(default)]
    pub next_message: u64,
}

impl GroupCounters {
    pub fn next_thread_id(&mut self, group_id: &GroupId) -> ThreadId {
        let id = Self::format_thread_id(group_id, self.next_thread);
        self.next_thread += 1;
        id
    }

    pub fn next_message_id(&mut self, group_id: &GroupId) -> MessageId {
        let id = Self::format_message_id(group_id, self.next_message);
        self.next_message += 1;
        id
    }

    pub fn peek_thread_counter(&self) -> u64 {
        self.next_thread
    }

    pub fn peek_message_counter(&self) -> u64 {
        self.next_message
    }

    fn format_thread_id(group_id: &GroupId, counter: u64) -> ThreadId {
        format!("{group_id}:thread:{counter}")
    }

    fn format_message_id(group_id: &GroupId, counter: u64) -> MessageId {
        format!("{group_id}:msg:{counter}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn thread_ids_are_unique_per_group() {
        let mut counters = GroupCounters::default();
        let group = "group-a".to_string();
        let first = counters.next_thread_id(&group);
        let second = counters.next_thread_id(&group);

        assert_ne!(first, second);
        assert_eq!(counters.peek_thread_counter(), 2);
        assert!(first.starts_with("group-a:thread:"));
    }

    #[test]
    fn message_ids_are_monotonic_per_group() {
        let mut counters = GroupCounters::default();
        let group = "group-a".to_string();

        let msg_a = counters.next_message_id(&group);
        let msg_b = counters.next_message_id(&group);

        assert_ne!(msg_a, msg_b);
        assert!(msg_a.starts_with("group-a:msg:"));
        assert!(msg_b.starts_with("group-a:msg:"));
        assert_eq!(counters.peek_message_counter(), 2);
    }

    fn sample_proposal() -> MembershipProposal {
        MembershipProposal {
            proposal_id: "p1".into(),
            candidate: "carol".into(),
            requested_role: "member".into(),
            proposer: "bob".into(),
            action: MembershipActionKind::Invite,
            eligible_voters: 5,
            ..MembershipProposal::default()
        }
    }

    #[test]
    fn dictator_rule_requires_single_signature() {
        let rule = DictatorRule::new("alice");
        let mut proposal = sample_proposal();

        let pending = rule.evaluate(&proposal);
        assert_eq!(pending.status, MembershipDecisionStatus::Pending);
        assert_eq!(pending.missing_signatures, vec!["alice".to_string()]);

        proposal.approvals.insert("alice".into());
        let decision = rule.evaluate(&proposal);
        assert_eq!(decision.status, MembershipDecisionStatus::Approved);
    }

    #[test]
    fn multi_dictator_rule_tracks_missing_signatures() {
        let dictators = HashSet::from_iter(["alice".into(), "dave".into()]);
        let rule = MultiDictatorRule::new(dictators, 2);
        let mut proposal = sample_proposal();
        proposal.approvals.insert("alice".into());

        let decision = rule.evaluate(&proposal);
        assert_eq!(decision.status, MembershipDecisionStatus::Pending);
        assert_eq!(decision.missing_signatures, vec!["dave".to_string()]);
    }

    #[test]
    fn tally_vote_rule_rejects_if_quorum_unreachable() {
        let rule = TallyVoteRule::new(0.6);
        let mut proposal = sample_proposal();
        proposal.eligible_voters = 5;
        proposal.approvals.insert("alice".into());
        proposal.rejections.insert("dave".into());
        proposal.rejections.insert("erin".into());
        proposal.rejections.insert("frank".into());

        let decision = rule.evaluate(&proposal);
        assert_eq!(decision.status, MembershipDecisionStatus::Rejected);
        assert_eq!(decision.reason.as_deref(), Some("quorum unreachable"));
    }

    #[test]
    fn token_rule_approves_once_threshold_met() {
        let rule = TokenThresholdRule::new(100, Some(0.6));
        let mut proposal = sample_proposal();
        proposal.token_support = 80;
        proposal.token_opposition = 40;
        let pending = rule.evaluate(&proposal);
        assert_eq!(pending.status, MembershipDecisionStatus::Pending);

        proposal.token_support = 120;
        let decision = rule.evaluate(&proposal);
        assert_eq!(decision.status, MembershipDecisionStatus::Approved);
    }

    #[test]
    fn compile_rules_from_config() {
        let configs = vec![
            MembershipRuleConfig::new("membership.rule.dictator", json!({ "dictator": "alice" })),
            MembershipRuleConfig::new("membership.rule.tally_vote", json!({ "quorum": 0.75 })),
        ];
        let rules = compile_membership_rules(&configs).expect("rules compile");
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn compile_unknown_rule_errors() {
        let config = MembershipRuleConfig::new("membership.rule.unknown", json!({}));
        match compile_membership_rules(&[config]) {
            Ok(_) => panic!("expected compile error for unknown rule"),
            Err(MembershipRuleError::UnknownRule(name)) => {
                assert_eq!(name, "membership.rule.unknown")
            }
            Err(other) => panic!("unexpected error pattern: {:?}", other),
        }
    }
}
