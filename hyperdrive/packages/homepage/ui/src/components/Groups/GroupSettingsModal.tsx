import React, { useMemo, useState } from 'react';
import { Chat } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import { useGroupStore } from '../../store/groups';
import { GroupPermissionKey, hasGroupPermission } from '../../constants/group';
import './GroupSettingsModal.css';

interface GroupSettingsModalProps {
  onClose: () => void;
}

const visibilityLabel = (visibility: Chat.GroupVisibility | null | undefined) => {
  switch (visibility) {
    case Chat.GroupVisibility.Public:
      return 'Public';
    case Chat.GroupVisibility.Private:
    default:
      return 'Private';
  }
};

const tierLabel = (tier: Chat.GroupTier | null | undefined) => {
  switch (tier) {
    case Chat.GroupTier.Hub:
      return 'Hub';
    case Chat.GroupTier.Subscriber:
    default:
      return 'Subscriber';
  }
};

const permissionLabels: { key: GroupPermissionKey; label: string }[] = [
  { key: 'SEND_MESSAGES', label: 'Send messages' },
  { key: 'CREATE_THREADS', label: 'Create threads' },
  { key: 'INVITE_MEMBERS', label: 'Invite members' },
  { key: 'MANAGE_ROLES', label: 'Manage roles' },
  { key: 'MANAGE_SETTINGS', label: 'Manage settings' },
];

const GroupSettingsModal: React.FC<GroupSettingsModalProps> = ({ onClose }) => {
  const { activeGroup, createGroupJoinLink } = useGroupStore();
  const { nodeId } = useChatStore();
  const [joinLink, setJoinLink] = useState<string | null>(null);
  const [joinError, setJoinError] = useState<string | null>(null);
  const [isCreatingLink, setIsCreatingLink] = useState(false);
  if (!activeGroup) return null;

  const roles = useMemo(() => Array.from(activeGroup.roles.values()), [activeGroup.roles]);
  const defaultRole =
    roles.find((r) => r.id === activeGroup.metadata.default_role_id) ||
    roles.find((r) => r.tier === Chat.GroupTier.Subscriber) ||
    roles[0];
  const member = nodeId ? activeGroup.members.get(nodeId) : undefined;
  const memberRole = member ? activeGroup.roles.get(member.role_id) : undefined;
  const canInvite =
    member?.status === Chat.MembershipStatus.Active &&
    hasGroupPermission(memberRole?.permissions as unknown as number, 'INVITE_MEMBERS');
  const isPublic = activeGroup.metadata.visibility === Chat.GroupVisibility.Public;

  const handleCreateLink = async () => {
    if (!isPublic || !canInvite) return;
    setIsCreatingLink(true);
    setJoinError(null);
    const link = await createGroupJoinLink(activeGroup.id);
    if (link) {
      setJoinLink(link);
    } else {
      setJoinError('Failed to create join link.');
    }
    setIsCreatingLink(false);
  };

  const handleCopyLink = () => {
    if (!joinLink) return;
    navigator.clipboard.writeText(joinLink).catch(() => {});
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content group-settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Group settings</h3>
          <button className="close-button" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="modal-body settings-body">
          <div className="settings-grid">
            <div className="settings-card">
              <div className="settings-label">Name</div>
              <div className="settings-value">{activeGroup.metadata.name || 'Untitled group'}</div>
            </div>
            <div className="settings-card">
              <div className="settings-label">Description</div>
              <div className="settings-value">
                {activeGroup.metadata.description || 'No description'}
              </div>
            </div>
            <div className="settings-card">
              <div className="settings-label">Visibility</div>
              <div className="settings-value">{visibilityLabel(activeGroup.metadata.visibility)}</div>
              <div className="settings-sub">Set when creating the group</div>
            </div>
            <div className="settings-card">
              <div className="settings-label">Governance</div>
              <div className="settings-value">Dictatorship (creator is Owner)</div>
              <div className="settings-sub">Advanced models coming in future versions</div>
            </div>
            {defaultRole && (
              <div className="settings-card">
                <div className="settings-label">Default role</div>
                <div className="settings-value">
                  {defaultRole.label} ({tierLabel(defaultRole.tier)})
                </div>
              </div>
            )}
          </div>

          {isPublic && (
            <div className="settings-section join-link-section">
              <div className="settings-section-header">
                <h4>Join link</h4>
                <span className="settings-sub">
                  Anyone with this link can join as a member.
                </span>
              </div>
              <div className="join-link-actions">
                <div className="join-link-row">
                  <button
                    className="join-link-button"
                    onClick={handleCreateLink}
                    disabled={!canInvite || isCreatingLink}
                  >
                    {isCreatingLink ? 'Creating…' : 'Create join link'}
                  </button>
                  {!canInvite && (
                    <span className="join-link-note">
                      You do not have permission to create join links.
                    </span>
                  )}
                </div>
                {joinLink && (
                  <div className="join-link-display">
                    <input
                      type="text"
                      value={joinLink}
                      readOnly
                      onClick={(e) => (e.target as HTMLInputElement).select()}
                    />
                    <button className="join-link-button secondary" onClick={handleCopyLink}>
                      Copy
                    </button>
                  </div>
                )}
                {joinError && <div className="join-link-error">{joinError}</div>}
              </div>
            </div>
          )}

          <div className="settings-section">
            <div className="settings-section-header">
              <h4>Roles & permissions</h4>
              <span className="settings-sub">
                Backed by the GroupPermissions bitset provided by the backend.
              </span>
            </div>
            <div className="roles-list">
              {roles.map((role) => (
                <div key={role.id} className="role-row">
                  <div className="role-main">
                    <div className="role-name">{role.label}</div>
                    <div className="role-sub">
                      {tierLabel(role.tier)} • {role.id}
                    </div>
                  </div>
                  <div className="role-perms">
                    {permissionLabels.map((perm) => {
                      const enabled = hasGroupPermission(
                        role.permissions as unknown as number,
                        perm.key,
                      );
                      return (
                        <span
                          key={perm.key}
                          className={`perm-chip ${enabled ? 'on' : 'off'}`}
                          title={perm.label}
                        >
                          {perm.label}
                        </span>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default GroupSettingsModal;
