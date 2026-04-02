use crate::{
    crdt::{
        DeliveryCursor, Group, GroupId, GroupTier, HubSyncState, MembershipStatus,
        SubscriberSyncState,
    },
    BrokerEnvelope, ChatState, ReplicationKind, ReplicationTask, SubscriberDeliveryEvent,
};
use hyperware_crdt::yrs::{Decode, Encode, StateVector};
use hyperware_process_lib::our;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

// Replication-specific constants
const SUBSCRIBER_LANE_TTL_SECS: u64 = 300;
const SUBSCRIBER_ACK_DEADLINE_SECS: u64 = 45;
const DELIVERY_DEDUPE_WINDOW_SECS: u64 = 120;
const DELIVERY_DEDUPE_LIMIT: usize = 2048;
const SUBSCRIBER_EVENT_BUFFER: usize = 256;

impl ChatState {
    fn dedupe_key(topic: &str, payload: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        topic.hash(&mut hasher);
        payload.hash(&mut hasher);
        hasher.finish()
    }

    fn register_delivery_fingerprint(&mut self, topic: &str, payload: &str, now: u64) -> bool {
        let key = Self::dedupe_key(topic, payload);
        if let Some(ts) = self.delivery_dedupe.get(&key) {
            if now.saturating_sub(*ts) < DELIVERY_DEDUPE_WINDOW_SECS {
                return true;
            }
        }
        self.delivery_dedupe.insert(key, now);
        self.prune_dedupe_cache(now);
        false
    }

    fn prune_dedupe_cache(&mut self, now: u64) {
        let cutoff = now.saturating_sub(DELIVERY_DEDUPE_WINDOW_SECS);
        self.delivery_dedupe.retain(|_, ts| *ts >= cutoff);
        if self.delivery_dedupe.len() > DELIVERY_DEDUPE_LIMIT {
            let overflow = self
                .delivery_dedupe
                .len()
                .saturating_sub(DELIVERY_DEDUPE_LIMIT);
            if overflow == 0 {
                return;
            }
            let mut oldest: Vec<(u64, u64)> = self
                .delivery_dedupe
                .iter()
                .map(|(k, ts)| (*k, *ts))
                .collect();
            oldest.sort_by_key(|(_, ts)| *ts);
            for (key, _) in oldest.into_iter().take(overflow) {
                self.delivery_dedupe.remove(&key);
            }
        }
    }

    fn record_subscriber_event(&mut self, event: SubscriberDeliveryEvent) {
        self.subscriber_events.push_back(event);
        if self.subscriber_events.len() > SUBSCRIBER_EVENT_BUFFER {
            let overflow = self.subscriber_events.len() - SUBSCRIBER_EVENT_BUFFER;
            for _ in 0..overflow {
                self.subscriber_events.pop_front();
            }
        }
    }

    pub(crate) fn enqueue_replication_task(&mut self, task: ReplicationTask) {
        self.replication_queue.push_back(task);
        self.wake_replication_worker();
    }

    pub fn wake_replication_worker(&self) {
        if let Some(tx) = &self.replication_wake_tx {
            tx.wake();
        }
    }

    pub(crate) fn next_ready_replication_task(&mut self, now: u64) -> Option<ReplicationTask> {
        let mut rotate = 0usize;
        while let Some(task) = self.replication_queue.pop_front() {
            if task.not_before <= now {
                return Some(task);
            }
            self.replication_queue.push_back(task);
            rotate += 1;
            if rotate >= self.replication_queue.len() {
                break;
            }
        }
        None
    }

    pub(crate) fn has_replication_task(
        &self,
        group_id: &GroupId,
        peer: &str,
        kind: ReplicationKind,
    ) -> bool {
        self.replication_queue.iter().any(|t| {
            &t.group_id == group_id
                && t.peer == peer
                && std::mem::discriminant(&t.kind) == std::mem::discriminant(&kind)
        })
    }

    pub(crate) fn peer_state_vector(&self, group_id: &GroupId, peer: &str) -> Option<StateVector> {
        let group = self.groups.get(group_id)?;
        let sync = group.hubs.sync.get(peer)?;
        let bytes = sync.last_state_vector.as_ref()?;
        StateVector::decode_v1(bytes).ok()
    }

    pub(crate) fn update_peer_state_vector(
        &mut self,
        group_id: &GroupId,
        peer: &str,
        sv: &StateVector,
    ) {
        if let Some(group) = self.groups.get_mut(group_id) {
            let now = Self::now_secs();
            group.hubs.upsert_sync(
                peer.to_string(),
                HubSyncState {
                    last_state_vector: Some(sv.encode_v1()),
                    last_seen_ts: now,
                    ..HubSyncState::default()
                },
            );
        }
    }

    pub(crate) fn update_local_hub_sync_state(&mut self, group_id: &GroupId, sv: &StateVector) {
        if let Some(group) = self.groups.get_mut(group_id) {
            let now = Self::now_secs();
            group.hubs.upsert_sync(
                our().node.clone(),
                HubSyncState {
                    last_state_vector: Some(sv.encode_v1()),
                    last_seen_ts: now,
                    ..HubSyncState::default()
                },
            );
        }
    }

    pub(crate) fn update_delivery_cursor(
        &mut self,
        group_id: &GroupId,
        peer: &str,
        is_hub: bool,
        queue_id: String,
        offset: Option<u64>,
    ) {
        if let Some(group) = self.groups.get_mut(group_id) {
            let now = Self::now_secs();
            let cursors = if is_hub {
                &mut group.delivery.hub_cursors
            } else {
                &mut group.delivery.subscriber_cursors
            };
            let entry = cursors
                .entry(peer.to_string())
                .or_insert_with(|| DeliveryCursor {
                    queue_id: queue_id.clone(),
                    last_offset: 0,
                    updated_at: now,
                });
            entry.queue_id = queue_id;
            let next = offset.unwrap_or_else(|| entry.last_offset.saturating_add(1));
            entry.last_offset = next;
            entry.updated_at = now;
        }
    }

    pub(crate) fn enqueue_replication_pushes(
        &mut self,
        group_id: &GroupId,
        state_vector_bytes: Vec<u8>,
    ) {
        let now = Self::now_secs();
        let Some(group) = self.groups.get(group_id).cloned() else {
            return;
        };
        // Hubs
        for hub in &group.hubs.active {
            if hub == &our().node {
                continue;
            }
            let since = self
                .peer_state_vector(group_id, hub)
                .map(|sv| sv.encode_v1());
            let hub_age = group
                .delivery
                .hub_cursors
                .get(hub)
                .map(|c| now.saturating_sub(c.updated_at))
                .unwrap_or(u64::MAX);
            let kind = if since.is_none() || hub_age > SUBSCRIBER_LANE_TTL_SECS {
                ReplicationKind::PushSnapshot
            } else {
                ReplicationKind::PushDelta
            };
            let task = ReplicationTask {
                group_id: group_id.clone(),
                peer: hub.clone(),
                kind,
                since,
                attempt: 0,
                not_before: now,
            };
            self.enqueue_replication_task(task);
        }

        // Subscribers - send to active members, plus removed members who haven't received
        // their removal notification yet (need one final push so they know they were removed)
        for (node_id, member) in &group.members {
            if node_id == &our().node {
                continue;
            }
            // Skip members who are pending (not yet fully joined)
            if member.status == MembershipStatus::Pending {
                continue;
            }
            // For removed members, only send if they haven't been notified yet
            // Check if we've already sent them an update after their removal
            if member.status == MembershipStatus::Removed {
                let last_cursor_update = group
                    .delivery
                    .subscriber_cursors
                    .get(node_id)
                    .map(|c| c.updated_at)
                    .unwrap_or(0);
                // If we've sent them an update after they were removed, skip them
                // (member.last_activity is set to the removal timestamp)
                if last_cursor_update >= member.last_activity {
                    continue;
                }
            }
            // For active Hub members, skip (they're handled in the Hubs loop above)
            let is_hub = group
                .roles
                .get(&member.role_id)
                .map(|role| role.tier == GroupTier::Hub)
                .unwrap_or(false);
            if is_hub && member.status == MembershipStatus::Active {
                continue;
            }
            let since = self
                .peer_state_vector(group_id, node_id)
                .map(|sv| sv.encode_v1());
            let cursor_age = group
                .delivery
                .subscriber_cursors
                .get(node_id)
                .map(|c| now.saturating_sub(c.updated_at))
                .unwrap_or(u64::MAX);
            let kind = if since.is_none() || cursor_age > SUBSCRIBER_LANE_TTL_SECS {
                ReplicationKind::PushSnapshot
            } else {
                ReplicationKind::PushDelta
            };
            self.enqueue_replication_task(ReplicationTask {
                group_id: group_id.clone(),
                peer: node_id.clone(),
                kind,
                since,
                attempt: 0,
                not_before: now,
            });
        }

        // Ensure we have our own sync recorded
        if let Ok(sv) = StateVector::decode_v1(&state_vector_bytes) {
            self.update_local_hub_sync_state(group_id, &sv);
        }
    }

    pub(crate) fn publish_broker_message(
        &mut self,
        topic: &str,
        payload: &str,
        acl_version: Option<u64>,
        kind: ReplicationKind,
    ) {
        let next = *self.broker_offsets.get(topic).unwrap_or(&0);
        let env = BrokerEnvelope {
            offset: next,
            payload: payload.to_string(),
            acl_version,
            kind,
            ts: Self::now_secs(),
        };
        let entry = self
            .broker_queues
            .entry(topic.to_string())
            .or_insert_with(VecDeque::new);
        entry.push_back(env);
        self.broker_offsets
            .insert(topic.to_string(), next.saturating_add(1));
        crate::log_debug!(
            "[BROKER] topic={} enqueued offset={} len={}",
            topic,
            next,
            entry.len()
        );
        self.wake_replication_worker();
    }

    pub(crate) fn consume_broker_topics(&mut self, max_per_topic: usize) -> usize {
        let mut applied = 0usize;
        let now = Self::now_secs();
        let topics: Vec<String> = self
            .groups
            .values()
            .flat_map(|g| {
                let mut t = Vec::new();
                if g.hubs.active.contains(&our().node) && !g.routing.hub_topic.is_empty() {
                    t.push(g.routing.hub_topic.clone());
                }
                // subscriber lane consumption if we are in subscribers
                if g.subscribers.entries.contains_key(&our().node)
                    && !g.routing.subscriber_topic.is_empty()
                {
                    t.push(g.routing.subscriber_topic.clone());
                }
                t
            })
            .collect();

        for topic in topics {
            let from = *self.broker_cursors.get(&topic).unwrap_or(&0);
            let envelopes: Vec<BrokerEnvelope> = self
                .broker_queues
                .get(&topic)
                .map(|q| {
                    q.iter()
                        .filter(|e| e.offset >= from)
                        .take(max_per_topic)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();

            for env in envelopes {
                if let Err(err) = self.apply_broker_envelope(&topic, &env, now) {
                    crate::log_debug!(
                        "[BROKER] topic={} offset={} apply error: {}",
                        topic,
                        env.offset,
                        err
                    );
                    continue;
                }
                applied += 1;
                self.broker_cursors.insert(topic.clone(), env.offset + 1);
            }
        }
        applied
    }

    #[cfg(feature = "test-helpers")]
    pub fn enqueue_stale_subscriber_replays(&mut self, now: u64) {
        self.enqueue_stale_subscriber_replays_inner(now);
    }

    #[cfg(not(feature = "test-helpers"))]
    pub(crate) fn enqueue_stale_subscriber_replays(&mut self, now: u64) {
        self.enqueue_stale_subscriber_replays_inner(now);
    }

    fn enqueue_stale_subscriber_replays_inner(&mut self, now: u64) {
        let local_node = our().node.clone();
        self.enqueue_stale_subscriber_replays_for_node(now, &local_node);
    }

    fn enqueue_stale_subscriber_replays_for_node(&mut self, now: u64, local_node: &str) {
        let groups: Vec<(GroupId, Group)> = self
            .groups
            .iter()
            .map(|(id, group)| (id.clone(), group.clone()))
            .collect();
        for (group_id, group) in groups {
            if group.routing.subscriber_topic.is_empty() {
                continue;
            }
            for (node_id, _) in group.subscribers.entries.iter() {
                if node_id == local_node {
                    continue;
                }
                if self.has_replication_task(&group_id, node_id, ReplicationKind::PushSnapshot)
                    || self.has_replication_task(&group_id, node_id, ReplicationKind::PushDelta)
                {
                    continue;
                }
                // Check the correct cursor based on whether node is a hub or subscriber.
                // Hubs use hub_cursors, subscribers use subscriber_cursors.
                let is_hub = group.hubs.active.contains(node_id);
                let cursor_age = if is_hub {
                    group.delivery.hub_cursors.get(node_id)
                } else {
                    group.delivery.subscriber_cursors.get(node_id)
                }
                .map(|cursor| now.saturating_sub(cursor.updated_at));
                let stale = cursor_age
                    .map(|age| age > SUBSCRIBER_ACK_DEADLINE_SECS)
                    .unwrap_or(true);
                if !stale {
                    continue;
                }
                let since = self
                    .peer_state_vector(&group_id, node_id)
                    .map(|sv| sv.encode_v1());
                let age = cursor_age.unwrap_or(0);
                let kind = if since.is_none() || age > SUBSCRIBER_LANE_TTL_SECS {
                    ReplicationKind::PushSnapshot
                } else {
                    ReplicationKind::PushDelta
                };
                let kind_for_log = kind.clone();
                self.enqueue_replication_task(ReplicationTask {
                    group_id: group_id.clone(),
                    peer: node_id.clone(),
                    kind,
                    since,
                    attempt: 0,
                    not_before: now,
                });
                self.replication_metrics.stale_replays =
                    self.replication_metrics.stale_replays.saturating_add(1);
                crate::log_debug!(
                    "[REPL][{}] queued {} replay to {} {} (age={}s)",
                    group_id,
                    match kind_for_log {
                        ReplicationKind::PushSnapshot => "snapshot",
                        _ => "delta",
                    },
                    if is_hub { "hub" } else { "subscriber" },
                    node_id,
                    age
                );
            }
        }
    }

    #[cfg(feature = "test-helpers")]
    pub fn apply_broker_envelope(
        &mut self,
        topic: &str,
        env: &BrokerEnvelope,
        now: u64,
    ) -> Result<(), String> {
        self.apply_broker_envelope_inner(topic, env, now)
    }

    #[cfg(not(feature = "test-helpers"))]
    fn apply_broker_envelope(
        &mut self,
        topic: &str,
        env: &BrokerEnvelope,
        now: u64,
    ) -> Result<(), String> {
        self.apply_broker_envelope_inner(topic, env, now)
    }

    fn apply_broker_envelope_inner(
        &mut self,
        topic: &str,
        env: &BrokerEnvelope,
        now: u64,
    ) -> Result<(), String> {
        // find group by topic
        let group_id = self
            .groups
            .iter()
            .find(|(_, g)| g.routing.hub_topic == topic || g.routing.subscriber_topic == topic)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| "no group for topic".to_string())?;
        let is_subscriber_topic = self
            .groups
            .get(&group_id)
            .map(|g| g.routing.subscriber_topic == topic)
            .unwrap_or(false);
        let created_at = if env.ts == 0 { now } else { env.ts };
        let age = now.saturating_sub(created_at);
        if age > SUBSCRIBER_ACK_DEADLINE_SECS {
            crate::log_debug!(
                "[BROKER][{}] delivery lag {}s topic={} offset={}",
                group_id,
                age,
                topic,
                env.offset
            );
        }
        if is_subscriber_topic {
            self.replication_metrics.last_subscriber_lag_secs = age;
            if age > SUBSCRIBER_LANE_TTL_SECS {
                self.replication_metrics.drops = self.replication_metrics.drops.saturating_add(1);
                crate::log_debug!(
                    "[BROKER][{}] drop stale subscriber envelope topic={} offset={} age={}s",
                    group_id,
                    topic,
                    env.offset,
                    age
                );
                return Ok(());
            }
            if self.register_delivery_fingerprint(topic, &env.payload, now) {
                self.replication_metrics.drops = self.replication_metrics.drops.saturating_add(1);
                crate::log_debug!(
                    "[BROKER][{}] drop duplicate subscriber envelope topic={} offset={}",
                    group_id,
                    topic,
                    env.offset
                );
                return Ok(());
            }
        } else {
            self.replication_metrics.last_lag_secs = age;
        }

        // ACL drift log
        if let Some(in_acl) = env.acl_version {
            if let Some(wl) = self.pubsub.whitelist(&group_id) {
                let local = wl.version();
                if local != in_acl {
                    crate::log_debug!(
                        "[BROKER][{}] ACL drift topic {} incoming={} local={}",
                        group_id,
                        topic,
                        in_acl,
                        local
                    );
                    self.replication_metrics.acl_drifts =
                        self.replication_metrics.acl_drifts.saturating_add(1);
                }
            }
        }

        self.apply_group_update_payload(
            &group_id,
            &env.payload,
            "broker_delivery",
            env.acl_version,
            is_subscriber_topic,
        )
        .map_err(|err| {
            self.replication_metrics.drops = self.replication_metrics.drops.saturating_add(1);
            err
        })?;
        // update cursors/delivery trackers
        let is_hub = self
            .groups
            .get(&group_id)
            .map(|g| g.routing.hub_topic == topic)
            .unwrap_or(false);
        self.update_delivery_cursor(
            &group_id,
            &our().node,
            is_hub,
            topic.to_string(),
            Some(env.offset),
        );
        // bump heartbeat
        if let Some(group) = self.groups.get_mut(&group_id) {
            group.hubs.upsert_sync(
                our().node.clone(),
                HubSyncState {
                    last_seen_ts: now,
                    ..HubSyncState::default()
                },
            );
            if is_subscriber_topic {
                let subscriber = group
                    .subscribers
                    .entries
                    .entry(our().node.clone())
                    .or_insert_with(SubscriberSyncState::default);
                subscriber.last_seen_ts = now;
                subscriber.last_state_vector = self
                    .group_doc_managers
                    .get(&group_id)
                    .and_then(|mgr| mgr.last_state_vector().map(|sv| sv.encode_v1()));
                if let ReplicationKind::PushSnapshot = env.kind {
                    let digest = format!("{:x}", Self::dedupe_key(topic, &env.payload));
                    subscriber.last_snapshot_digest = Some(digest);
                }
            }
        }
        if is_subscriber_topic {
            self.record_subscriber_event(SubscriberDeliveryEvent {
                group_id,
                topic: topic.to_string(),
                offset: env.offset,
                kind: env.kind.clone(),
                age_secs: age,
                recorded_at: now,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SUBSCRIBER_ACK_DEADLINE_SECS;
    use crate::crdt::{DeliveryCursor, Group, GroupRoutingConfig, SubscriberSyncState};
    use crate::ChatState;

    #[test]
    fn enqueue_stale_replays_uses_hub_cursors_for_hubs() {
        let mut state = ChatState::default();
        let group_id = "group:replay".to_string();
        let hub_node = "hub.node".to_string();
        let sub_node = "sub.node".to_string();

        let mut group = Group::default();
        group.routing = GroupRoutingConfig::for_group(&group_id);
        group
            .subscribers
            .entries
            .insert(hub_node.clone(), SubscriberSyncState::default());
        group
            .subscribers
            .entries
            .insert(sub_node.clone(), SubscriberSyncState::default());
        group.hubs.active.insert(hub_node.clone());

        let now: u64 = 1_000;
        group.delivery.hub_cursors.insert(
            hub_node.clone(),
            DeliveryCursor {
                queue_id: "hub-queue".to_string(),
                last_offset: 1,
                updated_at: now.saturating_sub(SUBSCRIBER_ACK_DEADLINE_SECS - 1),
            },
        );
        group.delivery.subscriber_cursors.insert(
            hub_node.clone(),
            DeliveryCursor {
                queue_id: "sub-queue".to_string(),
                last_offset: 1,
                updated_at: now.saturating_sub(SUBSCRIBER_ACK_DEADLINE_SECS + 1),
            },
        );
        group.delivery.subscriber_cursors.insert(
            sub_node.clone(),
            DeliveryCursor {
                queue_id: "sub-queue".to_string(),
                last_offset: 1,
                updated_at: now.saturating_sub(SUBSCRIBER_ACK_DEADLINE_SECS + 1),
            },
        );

        state.groups.insert(group_id, group);

        state.enqueue_stale_subscriber_replays_for_node(now, "local.node");

        let peers: Vec<String> = state
            .replication_queue
            .iter()
            .map(|t| t.peer.clone())
            .collect();
        assert!(peers.contains(&sub_node));
        assert!(!peers.contains(&hub_node));
    }
}
