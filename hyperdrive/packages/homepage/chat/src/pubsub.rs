use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

use hyperware_pubsub_core::whitelist::{
    NodeAccess, NodeId as BrokerNodeId, TopicPattern, Whitelist,
};

use crate::crdt::{Group, GroupId, GroupRoutingConfig, GroupTier, MembershipStatus, Role};

const AUDIENCE_HUB: &str = "hub";
const AUDIENCE_SUBSCRIBER: &str = "subscriber";
const FEATURE_CRDT: &str = "crdt";
const FEATURE_NOTIFY: &str = "notify";

#[derive(Default)]
pub struct PubSubRegistry {
    groups: HashMap<GroupId, GroupBroker>,
}

#[derive(Clone)]
pub struct GroupBroker {
    pub whitelist: Whitelist,
    pub routing: GroupRoutingConfig,
}

impl PubSubRegistry {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    pub fn rebuild_all(&mut self, groups: &HashMap<GroupId, Group>) {
        self.groups.clear();
        for (group_id, group) in groups {
            self.rebuild_group(group_id, group);
        }
    }

    pub fn rebuild_group(&mut self, group_id: &GroupId, group: &Group) {
        let routing = normalised_routing(group_id, group);
        let whitelist = build_whitelist(group, &routing);
        self.groups
            .insert(group_id.clone(), GroupBroker { whitelist, routing });
    }

    pub fn remove_group(&mut self, group_id: &GroupId) {
        self.groups.remove(group_id);
    }

    pub fn whitelist(&self, group_id: &GroupId) -> Option<&Whitelist> {
        self.groups.get(group_id).map(|broker| &broker.whitelist)
    }

    pub fn routing(&self, group_id: &GroupId) -> Option<&GroupRoutingConfig> {
        self.groups.get(group_id).map(|broker| &broker.routing)
    }
}

fn normalised_routing(group_id: &GroupId, group: &Group) -> GroupRoutingConfig {
    if group.routing.hub_topic.is_empty() || group.routing.subscriber_topic.is_empty() {
        GroupRoutingConfig::for_group(group_id)
    } else {
        group.routing.clone()
    }
}

fn build_whitelist(group: &Group, routing: &GroupRoutingConfig) -> Whitelist {
    let mut whitelist = Whitelist::new();
    for (node_id, member) in &group.members {
        if member.status != MembershipStatus::Active {
            continue;
        }
        if let Some(role) = group.roles.get(&member.role_id) {
            if let Some(access) = build_access_for_role(role, routing) {
                whitelist.grant(BrokerNodeId::new(node_id.clone()), access);
            }
        }
    }

    for node_id in group.subscribers.entries.keys() {
        // Only grant subscriber access if the node is an active member
        let is_active_member = group
            .members
            .get(node_id)
            .map(|m| m.status == MembershipStatus::Active)
            .unwrap_or(false);
        if !is_active_member {
            continue;
        }

        let mut access = NodeAccess {
            publish: Vec::new(),
            subscribe: Vec::new(),
            audiences: HashSet::from_iter([AUDIENCE_SUBSCRIBER.to_string()]),
            features: HashSet::from_iter([FEATURE_NOTIFY.to_string()]),
            expires_at: None,
        };
        if !routing.subscriber_topic.is_empty() {
            access
                .subscribe
                .push(TopicPattern::Exact(routing.subscriber_topic.clone()));
        }
        whitelist.grant(BrokerNodeId::new(node_id.clone()), access);
    }

    whitelist
}

fn build_access_for_role(role: &Role, routing: &GroupRoutingConfig) -> Option<NodeAccess> {
    let mut publish = Vec::new();
    let mut subscribe = Vec::new();
    let mut audiences = HashSet::new();
    let mut features = HashSet::new();

    match role.tier {
        GroupTier::Hub => {
            if !routing.hub_topic.is_empty() {
                publish.push(TopicPattern::Exact(routing.hub_topic.clone()));
                subscribe.push(TopicPattern::Exact(routing.hub_topic.clone()));
            }
            if !routing.subscriber_topic.is_empty() {
                subscribe.push(TopicPattern::Exact(routing.subscriber_topic.clone()));
            }
            audiences.insert(AUDIENCE_HUB.to_string());
            features.insert(FEATURE_CRDT.to_string());
        }
        GroupTier::Subscriber => {
            if !routing.subscriber_topic.is_empty() {
                subscribe.push(TopicPattern::Exact(routing.subscriber_topic.clone()));
            }
            audiences.insert(AUDIENCE_SUBSCRIBER.to_string());
            features.insert(FEATURE_NOTIFY.to_string());
        }
    }

    Some(NodeAccess {
        publish,
        subscribe,
        audiences,
        features,
        expires_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::{GroupMember, GroupPermissions, SubscriberSyncState};
    use hyperware_pubsub_core::{whitelist::NodeId as BrokerNodeId, TopicId as BrokerTopicId};
    use std::time::SystemTime;

    fn sample_group(group_id: &str) -> Group {
        let mut group = Group::default();
        group.routing = GroupRoutingConfig::for_group(&group_id.to_string());

        let mut hub_permissions = GroupPermissions::empty();
        hub_permissions.insert(GroupPermissions::SEND_MESSAGES);
        hub_permissions.insert(GroupPermissions::CREATE_THREADS);
        hub_permissions.insert(GroupPermissions::INVITE_MEMBERS);
        hub_permissions.insert(GroupPermissions::MANAGE_ROLES);

        let mut member_permissions = GroupPermissions::empty();
        member_permissions.insert(GroupPermissions::SEND_MESSAGES);
        member_permissions.insert(GroupPermissions::CREATE_THREADS);

        let hub_role_id = "role:hub".to_string();
        let member_role_id = "role:member".to_string();

        group.roles.insert(
            hub_role_id.clone(),
            Role::new(hub_role_id.clone(), "Hub", hub_permissions, GroupTier::Hub),
        );
        group.roles.insert(
            member_role_id.clone(),
            Role::new(
                member_role_id.clone(),
                "Member",
                member_permissions,
                GroupTier::Subscriber,
            ),
        );

        group.members.insert(
            "hub.node".into(),
            GroupMember::new("hub.node", hub_role_id, MembershipStatus::Active, 0),
        );
        group.members.insert(
            "member.node".into(),
            GroupMember::new(
                "member.node",
                member_role_id.clone(),
                MembershipStatus::Active,
                0,
            ),
        );
        group
            .subscribers
            .entries
            .insert("member.node".into(), SubscriberSyncState::default());
        group.hubs.active.insert("hub.node".into());

        group
    }

    #[test]
    fn registry_projects_acl_for_hubs_and_subscribers() {
        let group_id = "group:test".to_string();
        let group = sample_group(&group_id);
        let mut registry = PubSubRegistry::new();
        registry.rebuild_group(&group_id, &group);

        let whitelist = registry.whitelist(&group_id).expect("whitelist exists");
        let hub_topic = BrokerTopicId::new(group.routing.hub_topic.clone());
        let subscriber_topic = BrokerTopicId::new(group.routing.subscriber_topic.clone());

        let hub_node = BrokerNodeId::new("hub.node".to_string());
        let member_node = BrokerNodeId::new("member.node".to_string());

        assert!(whitelist
            .publish_scope(&hub_node, &hub_topic, SystemTime::now())
            .is_some());
        assert!(whitelist
            .subscribe_scope(&hub_node, &hub_topic, SystemTime::now())
            .is_some());
        assert!(whitelist
            .subscribe_scope(&member_node, &subscriber_topic, SystemTime::now())
            .is_some());
    }

    #[test]
    fn revoked_members_lose_subscriber_access() {
        let group_id = "group:test".to_string();
        let mut group = sample_group(&group_id);
        let mut registry = PubSubRegistry::new();

        registry.rebuild_group(&group_id, &group);
        let subscriber_topic = BrokerTopicId::new(group.routing.subscriber_topic.clone());
        let member_node = BrokerNodeId::new("member.node".to_string());

        assert!(registry
            .whitelist(&group_id)
            .expect("whitelist exists")
            .subscribe_scope(&member_node, &subscriber_topic, SystemTime::now())
            .is_some());

        if let Some(member) = group.members.get_mut("member.node") {
            member.status = MembershipStatus::Removed;
        }
        group.subscribers.entries.remove("member.node");
        registry.rebuild_group(&group_id, &group);

        assert!(registry
            .whitelist(&group_id)
            .expect("whitelist exists after rebuild")
            .subscribe_scope(&member_node, &subscriber_topic, SystemTime::now())
            .is_none());
    }

    /// Tests that even if subscribers.entries is NOT cleaned up (race condition),
    /// a removed member still loses access because we check membership status.
    #[test]
    fn removed_member_loses_access_even_with_stale_subscriber_entry() {
        let group_id = "group:test".to_string();
        let mut group = sample_group(&group_id);
        let mut registry = PubSubRegistry::new();

        registry.rebuild_group(&group_id, &group);
        let subscriber_topic = BrokerTopicId::new(group.routing.subscriber_topic.clone());
        let member_node = BrokerNodeId::new("member.node".to_string());

        // Initially, member has access
        assert!(registry
            .whitelist(&group_id)
            .expect("whitelist exists")
            .subscribe_scope(&member_node, &subscriber_topic, SystemTime::now())
            .is_some());

        // Mark member as Removed, but DON'T clean up subscribers.entries
        // This simulates a race condition or bug where subscriber entry isn't cleaned
        if let Some(member) = group.members.get_mut("member.node") {
            member.status = MembershipStatus::Removed;
        }
        // Note: we deliberately do NOT call group.subscribers.entries.remove()

        registry.rebuild_group(&group_id, &group);

        // Member should still lose access because we check membership status
        assert!(registry
            .whitelist(&group_id)
            .expect("whitelist exists after rebuild")
            .subscribe_scope(&member_node, &subscriber_topic, SystemTime::now())
            .is_none());
    }
}
