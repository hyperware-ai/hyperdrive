import React, { useEffect, useMemo, useState } from 'react';
import { Chat } from '#caller-utils';
import { useGroupStore } from '../../store/groups';
import { useChatStore } from '../../store/chat';
import { hasGroupPermission } from '../../constants/group';
import GroupMessage from './GroupMessage';
import GroupMessageInput from './GroupMessageInput';
import GroupMembersModal from './GroupMembersModal';
import GroupReplicationPanel from './GroupReplicationPanel';
import GroupSettingsModal from './GroupSettingsModal';
import ThreadsDropdown from './ThreadsDropdown';
import ThreadView from './ThreadView';
import './GroupView.css';

type ThreadWithId = Chat.Thread & { id: string };

const GroupView: React.FC = () => {
  const {
    activeGroup,
    activeGroupId,
    activeThreadId,
    draftThread,
    setActiveThread,
    clearActiveGroup,
    startDraftThread,
    sendMessage,
    editMessage,
    deleteMessage,
    toggleReaction,
    refreshActiveGroup,
    subscriberEvents,
    replication,
    fetchSubscriberEvents,
    whitelists,
    fetchWhitelist,
    replyingTo,
    setReplyingTo,
    editingMessage,
    setEditingMessage,
    markGroupAsRead,
    groupNotify,
    updateGroupSettings,
    jumpToMessageId,
    setJumpToMessageId,
  } = useGroupStore();
  const { nodeId } = useChatStore();
  const [showMembers, setShowMembers] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showThreads, setShowThreads] = useState(false);
  const [showMenu, setShowMenu] = useState(false);
  const [isFetchingWhitelist, setIsFetchingWhitelist] = useState(false);

  useEffect(() => {
    if (!activeGroup) return;
    const interval = setInterval(() => refreshActiveGroup(), 15000);
    return () => clearInterval(interval);
  }, [activeGroup?.id, refreshActiveGroup]);

  useEffect(() => {
    const handleVisibility = () => {
      if (!document.hidden && activeGroup) {
        refreshActiveGroup();
      }
    };
    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  }, [activeGroup?.id, refreshActiveGroup]);

  useEffect(() => {
    if (!activeGroup) return;
    fetchSubscriberEvents();
    const interval = setInterval(() => fetchSubscriberEvents(), 20000);
    return () => clearInterval(interval);
  }, [activeGroup?.id, fetchSubscriberEvents]);

  // Mark group as read when entering the group view
  useEffect(() => {
    if (activeGroupId) {
      markGroupAsRead(activeGroupId);
    }
  }, [activeGroupId, markGroupAsRead]);

  if (!activeGroup) return null;

  const threads = useMemo<ThreadWithId[]>(() => {
    const list = Array.from(activeGroup.threads.entries()).map(([id, thread]) => ({
      ...thread,
      id,
    }));
    return list.sort((a, b) => {
      const aTs = a.summary?.last_activity ?? 0;
      const bTs = b.summary?.last_activity ?? 0;
      if (aTs !== bTs) return bTs - aTs;
      return a.depth - b.depth;
    });
  }, [activeGroup.threads]);

  const selectedThreadId = activeThreadId || activeGroup.rootThreadId;
  useEffect(() => {
    if (!threads.length) return;
    const exists = threads.some((t) => t.id === selectedThreadId);
    if (!exists) {
      setActiveThread(threads[0].id);
    }
  }, [selectedThreadId, setActiveThread, threads]);

  const member = nodeId ? activeGroup.members.get(nodeId) : undefined;
  const role = member ? activeGroup.roles.get(member.role_id) : undefined;

  const canSend =
    member?.status === Chat.MembershipStatus.Active &&
    !!role &&
    hasGroupPermission(role.permissions as unknown as number, 'SEND_MESSAGES');
  const canCreateThread =
    member?.status === Chat.MembershipStatus.Active &&
    !!role &&
    hasGroupPermission(role.permissions as unknown as number, 'CREATE_THREADS');
  const canInvite =
    member?.status === Chat.MembershipStatus.Active &&
    !!role &&
    hasGroupPermission(role.permissions as unknown as number, 'INVITE_MEMBERS');
  const whitelist = whitelists[activeGroup.id];

  const isRemoved = member?.status === Chat.MembershipStatus.Removed;

  let disabledReason: string | undefined;
  if (!member) disabledReason = 'You are not a member of this group.';
  else if (isRemoved)
    disabledReason = 'You are no longer a member of this group.';
  else if (member.status === Chat.MembershipStatus.Pending)
    disabledReason = 'Membership pending approval.';
  else if (member.status !== Chat.MembershipStatus.Active)
    disabledReason = 'Membership not active.';
  else if (!canSend)
    disabledReason = 'You do not have permission to post.';

  const groupEvents = subscriberEvents.filter(
    (evt) => evt.group_id === activeGroup.id,
  );
  const replicationState = replication[activeGroup.id];

  const handleCreateThread = () => {
    if (!canCreateThread) return;
    setShowThreads(false);
    // Start a draft thread - actual creation happens when first message is sent
    startDraftThread(activeGroup.rootThreadId || '', null);
  };

  const handleStartThreadFromMessage = (parentThreadId: string, rootMessageId?: string) => {
    if (!canCreateThread) return;
    // Start a draft thread - actual creation happens when first message is sent
    startDraftThread(parentThreadId, rootMessageId || null);
  };

  const handleFetchWhitelist = async () => {
    if (isFetchingWhitelist) return;
    setIsFetchingWhitelist(true);
    await fetchWhitelist(activeGroup.id);
    setIsFetchingWhitelist(false);
  };

  const handleReply = (messageId: string) => {
    const message = activeGroup?.messages.find((m) => m.id === messageId);
    if (message) {
      setReplyingTo(message);
    }
  };

  const handleCancelReply = () => {
    setReplyingTo(null);
  };

  const handleReact = (messageId: string, emoji: string) => {
    toggleReaction(messageId, emoji);
  };

  return (
    <div className="group-view">
      <header className="group-header">
        <button className="group-back" onClick={clearActiveGroup}>
          ←
        </button>
        <div className="group-header-info">
          <div className="group-header-name">{activeGroup.metadata.name}</div>
        </div>
        <div className="group-menu-wrapper">
          <button className="group-menu-btn" onClick={() => setShowMenu(!showMenu)} aria-label="Group menu">
            ⚙️
          </button>
          {showMenu && (
            <>
              <div className="group-menu-overlay" onClick={() => setShowMenu(false)} />
              <div className="group-menu-dropdown">
                <button
                  onClick={() => {
                    setShowMenu(false);
                    setShowThreads(true);
                  }}
                >
                  Threads
                </button>
                <button
                  onClick={() => {
                    setShowMenu(false);
                    setShowMembers(true);
                  }}
                >
                  Members
                </button>
                <button
                  onClick={() => {
                    if (activeGroupId) {
                      const currentNotify = groupNotify[activeGroupId] ?? true;
                      updateGroupSettings(activeGroupId, { notify: !currentNotify });
                    }
                    setShowMenu(false);
                  }}
                >
                  {activeGroupId && groupNotify[activeGroupId] === false
                    ? 'Enable Notifications'
                    : 'Mute Notifications'}
                </button>
              </div>
            </>
          )}
          {showThreads && (
            <ThreadsDropdown
              threads={threads}
              activeThreadId={selectedThreadId}
              rootThreadId={activeGroup.rootThreadId}
              onSelect={(id) => setActiveThread(id)}
              onClose={() => setShowThreads(false)}
              canCreateThread={canCreateThread}
              onCreateThread={handleCreateThread}
            />
          )}
        </div>
      </header>

      {isRemoved && (
        <div className="group-removed-banner">
          You are no longer a member of this group.
        </div>
      )}

      <GroupReplicationPanel
        replication={replicationState}
        whitelist={whitelist}
      />

      <section className="group-thread-section">
        <div className="group-thread-layout">
          <div className="thread-view-column">
            <ThreadView
              threadId={draftThread ? null : selectedThreadId}
              threads={threads}
              messages={activeGroup.messages}
              currentNode={nodeId}
              canSend={!!canSend}
              disabledReason={disabledReason}
              canStartThread={canCreateThread}
              replyingTo={replyingTo}
              draftThread={draftThread}
              editingMessage={editingMessage}
              onSend={sendMessage}
              onStartThread={canCreateThread ? handleStartThreadFromMessage : undefined}
              onOpenThread={(id) => setActiveThread(id)}
              onReply={handleReply}
              onCancelReply={handleCancelReply}
              onSetEditingMessage={setEditingMessage}
              onCancelEdit={() => setEditingMessage(null)}
              onEdit={editMessage}
              onDelete={deleteMessage}
              onReact={handleReact}
              jumpToMessageId={jumpToMessageId}
              onJumpComplete={() => setJumpToMessageId(null)}
            />
          </div>
        </div>
      </section>

      {showMembers && <GroupMembersModal onClose={() => setShowMembers(false)} />}
      {showSettings && <GroupSettingsModal onClose={() => setShowSettings(false)} />}
    </div>
  );
};

export default GroupView;
