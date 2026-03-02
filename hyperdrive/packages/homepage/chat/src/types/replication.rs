use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};

use crate::crdt::GroupId;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReplicationMetrics {
    #[serde(default)]
    pub acl_skips: u64,
    #[serde(default)]
    pub retries: u64,
    #[serde(default)]
    pub drops: u64,
    #[serde(default)]
    pub stale_replays: u64,
    #[serde(default)]
    pub last_lag_secs: u64,
    #[serde(default)]
    pub last_subscriber_lag_secs: u64,
    #[serde(default)]
    pub acl_drifts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriberDeliveryEvent {
    pub group_id: GroupId,
    pub topic: String,
    pub offset: u64,
    pub kind: ReplicationKind,
    pub age_secs: u64,
    pub recorded_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReplicationKind {
    PushDelta,
    PushSnapshot,
    PullSnapshot,
    PullDelta,
}

impl Default for ReplicationKind {
    fn default() -> Self {
        ReplicationKind::PushDelta
    }
}

#[derive(Clone, Debug)]
pub struct ReplicationTask {
    pub group_id: GroupId,
    pub peer: String,
    pub kind: ReplicationKind,
    pub since: Option<Vec<u8>>,
    pub attempt: u32,
    pub not_before: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerEnvelope {
    pub offset: u64,
    pub payload: String,
    #[serde(default)]
    pub acl_version: Option<u64>,
    #[serde(default)]
    pub kind: ReplicationKind,
    #[serde(default)]
    pub ts: u64,
}

#[derive(Clone)]
pub struct ReplicationTx {
    sender: UnboundedSender<ReplicationTask>,
}

impl ReplicationTx {
    pub fn new() -> (Self, UnboundedReceiver<ReplicationTask>) {
        let (sender, receiver) = mpsc::unbounded();
        (ReplicationTx { sender }, receiver)
    }

    pub fn unbounded_send(
        &self,
        task: ReplicationTask,
    ) -> Result<(), mpsc::TrySendError<ReplicationTask>> {
        self.sender.unbounded_send(task)
    }
}

#[derive(Clone)]
pub struct ReplicationWakeTx {
    sender: UnboundedSender<()>,
}

pub struct ReplicationWakeRx {
    receiver: UnboundedReceiver<()>,
}

impl ReplicationWakeTx {
    pub fn new() -> (Self, ReplicationWakeRx) {
        let (sender, receiver) = mpsc::unbounded();
        (ReplicationWakeTx { sender }, ReplicationWakeRx { receiver })
    }

    pub fn wake(&self) {
        let _ = self.sender.unbounded_send(());
    }
}

impl ReplicationWakeRx {
    pub fn into_stream(self) -> UnboundedReceiver<()> {
        self.receiver
    }
}
