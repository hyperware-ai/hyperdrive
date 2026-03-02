import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Chat as api } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import { useGroupStore } from '../../store/groups';
import { useSpiderStore } from '../../store/spider';
import ChatSearch from '../Chats/ChatSearch';
import ChatItemMenu from './ChatItemMenu';
import { getChatDisplayName, getChatNodeSubtitle } from '../../utils/chatDisplay';
import './UnifiedMessages.css';

type UnifiedItem = {
  id: string;
  kind: 'dm' | 'group' | 'spider';
  title: string;
  subtitle: string;
  threadPath: string | null;
  lastActivity: number;
  unread: number;
  isPinned?: boolean;
  onClick: () => void;
  // For context menu actions
  chatId?: string;
  groupId?: string;
};

// LocalStorage key for pinned items
const PINNED_ITEMS_KEY = 'hyperdrive-pinned-chats';
type ChooserStep = 'chooser' | 'dm' | 'group';

interface UnifiedMessagesProps {
  showSpiderChat: boolean;
  setShowSpiderChat: (show: boolean) => void;
}

const UnifiedMessages: React.FC<UnifiedMessagesProps> = ({
  showSpiderChat,
  setShowSpiderChat,
}) => {
  const { chats, searchIndex, connectionStatus, setActiveChat, setJumpToMessageId, deleteChat, createChat } = useChatStore();
  const {
    groups,
    groupPreviews,
    groupUnread,
    loadGroups,
    fetchReplicationState,
    openGroup,
    setActiveThread,
    setJumpToMessageId: setGroupJumpToMessageId,
    isLoading,
    error,
    leaveGroup,
    createGroup,
  } = useGroupStore();

  const { messages: spiderMessages, checkStatus } = useSpiderStore();

  const [query, setQuery] = useState('');
  const [searchResults, setSearchResults] = useState<api.SearchResultItem[] | null>(null);
  const [showChooser, setShowChooser] = useState(false);
  const [chooserStep, setChooserStep] = useState<ChooserStep>('chooser');
  const [chooserError, setChooserError] = useState<string | null>(null);
  const [dmCounterparty, setDmCounterparty] = useState('');
  const [groupName, setGroupName] = useState('');
  const [groupDescription, setGroupDescription] = useState('');
  const [groupVisibility, setGroupVisibility] = useState<api.GroupVisibility>(
    api.GroupVisibility.Private,
  );
  const [isSubmittingDm, setIsSubmittingDm] = useState(false);
  const [isSubmittingGroup, setIsSubmittingGroup] = useState(false);
  const chooserOpenedAtRef = useRef(0);

  // Context menu state
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number } | null>(null);
  const [menuItem, setMenuItem] = useState<UnifiedItem | null>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const touchStartRef = useRef<{ x: number; y: number } | null>(null);

  // Pinned items state
  const [pinnedItems, setPinnedItems] = useState<string[]>(() => {
    try {
      const stored = localStorage.getItem(PINNED_ITEMS_KEY);
      return stored ? JSON.parse(stored) : ['spider'];
    } catch {
      return ['spider'];
    }
  });

  // Save pinned items to localStorage
  useEffect(() => {
    localStorage.setItem(PINNED_ITEMS_KEY, JSON.stringify(pinnedItems));
  }, [pinnedItems]);

  // Clean up timer on unmount
  useEffect(() => {
    return () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  // Check Spider availability on mount
  useEffect(() => {
    checkStatus();
  }, [checkStatus]);

  // Keep group data fresh when we land on the unified view
  useEffect(() => {
    if (connectionStatus === 'connected') {
      loadGroups();
      fetchReplicationState(null);
    }
  }, [connectionStatus, loadGroups, fetchReplicationState]);

  // Filter DMs + groups using the search index
  useEffect(() => {
    let cancelled = false;

    const runSearch = async () => {
      try {
        if (query.trim()) {
          const results = await searchIndex(query, {
            scope: api.SearchScope.All,
            limit: 100,
          });
          if (!cancelled) setSearchResults(results);
        } else {
          setSearchResults(null);
        }
      } catch (err) {
        if (!cancelled) setSearchResults([]);
      }
    };

    runSearch();
    return () => {
      cancelled = true;
    };
  }, [query, searchIndex]);

  const formatThreadPath = (threadId?: string | null) => {
    if (!threadId) return null;
    const parts = threadId.split(':');
    const suffix = parts[parts.length - 1] || threadId;
    return `Thread ${suffix}`;
  };

  const getLatestChatActivity = useCallback((chat: api.Chat) => {
    const lastMessageTimestamp =
      chat.messages.length > 0 ? chat.messages[chat.messages.length - 1].timestamp || 0 : 0;
    return Math.max(chat.last_activity || 0, lastMessageTimestamp);
  }, []);

  const unifiedItems = useMemo(() => {
    if (searchResults) {
      const chatById = new Map(chats.map((chat) => [chat.id, chat]));
      const groupById = new Map(groups.map((group) => [group.group_id, group]));

      const items = searchResults.reduce<UnifiedItem[]>((acc, result, idx) => {
          if (
            result.kind === api.SearchResultKind.ChatSummary ||
            result.kind === api.SearchResultKind.ChatMessage
          ) {
            const chatId = result.chat_id ?? '';
            const chat = chatById.get(chatId);
            if (!chat) return acc;
            const lastMessage = chat.messages[chat.messages.length - 1];
            const subtitle =
              result.snippet ?? lastMessage?.content ?? 'No messages yet';
            const lastActivity = result.timestamp ?? getLatestChatActivity(chat);
            acc.push({
              id: result.message_id ? `dm-msg-${result.message_id}` : `dm-${chat.id}-${idx}`,
              kind: 'dm' as const,
              title: getChatDisplayName(chat) || result.title || 'Direct message',
              subtitle,
              threadPath: getChatNodeSubtitle(chat),
              lastActivity,
              unread: chat.unread_count,
              onClick: () => {
                if (result.message_id) {
                  setJumpToMessageId(result.message_id);
                }
                setActiveChat(chat);
              },
            });
            return acc;
          }

          if (
            result.kind === api.SearchResultKind.GroupSummary ||
            result.kind === api.SearchResultKind.GroupMessage
          ) {
            const groupId = result.group_id ?? '';
            const group = groupById.get(groupId);
            if (!group) return acc;
            const preview = groupPreviews[group.group_id];
            const subtitle =
              result.snippet ??
              preview?.text ??
              group.metadata?.description ??
              'No messages yet';
            const lastActivity =
              result.timestamp ??
              preview?.timestamp ??
              group.metadata?.updated_at ??
              0;
            const threadPath =
              result.thread_id ? formatThreadPath(result.thread_id) : preview?.threadPath || null;
            acc.push({
              id: result.message_id
                ? `group-msg-${result.message_id}`
                : `group-${group.group_id}-${idx}`,
              kind: 'group' as const,
              title: group.metadata?.name || result.title || 'Untitled group',
              subtitle,
              threadPath,
              lastActivity,
              unread: groupUnread[group.group_id] || 0,
              onClick: () => {
                void openGroup(group.group_id).then(() => {
                  if (result.thread_id) {
                    setActiveThread(result.thread_id);
                  }
                  if (result.message_id) {
                    setGroupJumpToMessageId(result.message_id);
                  }
                });
              },
            });
            return acc;
          }
          return acc;
        }, []);

      return items;
    }

    const dmItems: UnifiedItem[] = chats.map((chat) => {
      const lastMessage = chat.messages[chat.messages.length - 1];
      const lastActivity = getLatestChatActivity(chat);
      const preview = lastMessage?.content || 'No messages yet';
      const itemId = `dm-${chat.id}`;
      return {
        id: itemId,
        kind: 'dm' as const,
        title: getChatDisplayName(chat) || 'Direct message',
        subtitle: preview,
        threadPath: getChatNodeSubtitle(chat),
        lastActivity,
        unread: chat.unread_count,
        isPinned: pinnedItems.includes(itemId),
        onClick: () => setActiveChat(chat),
        chatId: chat.id,
      };
    });

    const groupItems: UnifiedItem[] = groups.map((group) => {
      const preview = groupPreviews[group.group_id];
      const lastActivity =
        preview?.timestamp || group.metadata?.updated_at || Math.floor(Date.now() / 1000);
      const subtitle = preview?.text || 'No messages yet';
      const threadPath = preview?.threadPath || null;
      const itemId = `group-${group.group_id}`;
      return {
        id: itemId,
        kind: 'group' as const,
        title: group.metadata?.name || 'Untitled group',
        subtitle,
        threadPath,
        lastActivity,
        onClick: () => openGroup(group.group_id),
        unread: groupUnread[group.group_id] || 0,
        isPinned: pinnedItems.includes(itemId),
        groupId: group.group_id,
      };
    });

    // Create Spider item (always pinned at top)
    const lastSpiderMessage = spiderMessages[spiderMessages.length - 1];
    const spiderItem: UnifiedItem = {
      id: 'spider',
      kind: 'spider' as const,
      title: 'Spider',
      subtitle: lastSpiderMessage?.content?.text?.slice(0, 50) || 'AI Assistant - Ask anything!',
      threadPath: null,
      lastActivity: lastSpiderMessage?.timestamp || 0,
      unread: 0,
      isPinned: pinnedItems.includes('spider'),
      onClick: () => setShowSpiderChat(true),
    };

    // Sort: pinned items first, then by lastActivity
    const allItems = [spiderItem, ...dmItems, ...groupItems];
    const pinnedList = allItems.filter(item => item.isPinned);
    const unpinnedList = allItems.filter(item => !item.isPinned);

    pinnedList.sort((a, b) => {
      // Spider always first among pinned
      if (a.id === 'spider') return -1;
      if (b.id === 'spider') return 1;
      return (b.lastActivity || 0) - (a.lastActivity || 0);
    });

    unpinnedList.sort((a, b) => (b.lastActivity || 0) - (a.lastActivity || 0));

    return [...pinnedList, ...unpinnedList];
  }, [
    searchResults,
    chats,
    groups,
    groupPreviews,
    groupUnread,
    openGroup,
    setActiveChat,
    setActiveThread,
    spiderMessages,
    pinnedItems,
    getLatestChatActivity,
  ]);

  const formatTime = (timestamp?: number | null) => {
    if (!timestamp) return '';
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    
    if (days === 0) {
      return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
    } else if (days === 1) {
      return 'Yesterday';
    } else if (days < 7) {
      return date.toLocaleDateString('en-US', { weekday: 'short' });
    } else {
      return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    }
  };

  useEffect(() => {
    if (showChooser) {
      chooserOpenedAtRef.current = Date.now();
    }
  }, [showChooser]);

  const resetChooserState = useCallback(() => {
    setChooserStep('chooser');
    setChooserError(null);
    setDmCounterparty('');
    setGroupName('');
    setGroupDescription('');
    setGroupVisibility(api.GroupVisibility.Private);
    setIsSubmittingDm(false);
    setIsSubmittingGroup(false);
  }, []);

  const openChooser = useCallback(() => {
    resetChooserState();
    setShowChooser(true);
  }, [resetChooserState]);

  const closeChooser = useCallback(() => {
    setShowChooser(false);
    resetChooserState();
  }, [resetChooserState]);

  const handleChooserStep = useCallback((step: ChooserStep) => {
    setChooserError(null);
    setChooserStep(step);
  }, []);

  const handleDmSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const counterparty = dmCounterparty.trim();
      if (!counterparty) {
        setChooserError('Node address is required.');
        return;
      }

      try {
        setChooserError(null);
        setIsSubmittingDm(true);
        await createChat(counterparty);
        closeChooser();
      } catch (err) {
        setChooserError('Failed to start chat.');
      } finally {
        setIsSubmittingDm(false);
      }
    },
    [closeChooser, createChat, dmCounterparty],
  );

  const handleGroupSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const name = groupName.trim();
      if (!name) {
        setChooserError('Group name is required.');
        return;
      }

      try {
        setChooserError(null);
        setIsSubmittingGroup(true);
        const groupId = await createGroup({
          name,
          description: groupDescription.trim() || undefined,
          visibility: groupVisibility,
          rootThreadTitle: null,
        });
        if (groupId) {
          closeChooser();
          return;
        }
        setChooserError('Failed to create group.');
      } catch (err) {
        setChooserError('Failed to create group.');
      } finally {
        setIsSubmittingGroup(false);
      }
    },
    [closeChooser, createGroup, groupDescription, groupName, groupVisibility],
  );

  const handleChooserOverlayClick = (event: React.MouseEvent<HTMLDivElement>) => {
    // Ignore the delayed click that can land on the overlay right after opening on Android.
    if (Date.now() - chooserOpenedAtRef.current < 350) {
      event.stopPropagation();
      return;
    }
    closeChooser();
  };

  // Context menu handlers
  const handleContextMenu = useCallback((e: React.MouseEvent, item: UnifiedItem) => {
    e.preventDefault();
    e.stopPropagation();
    setMenuPosition({ x: e.clientX, y: e.clientY });
    setMenuItem(item);
  }, []);

  const handleTouchStart = useCallback((e: React.TouchEvent, item: UnifiedItem) => {
    const touch = e.touches[0];
    touchStartRef.current = { x: touch.clientX, y: touch.clientY };

    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
    }

    longPressTimerRef.current = setTimeout(() => {
      setMenuPosition({ x: touch.clientX, y: touch.clientY });
      setMenuItem(item);
      if ('vibrate' in navigator) {
        navigator.vibrate(10);
      }
    }, 500);
  }, []);

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    if (!touchStartRef.current) return;

    const touch = e.touches[0];
    const deltaX = Math.abs(touch.clientX - touchStartRef.current.x);
    const deltaY = Math.abs(touch.clientY - touchStartRef.current.y);

    if (deltaX > 10 || deltaY > 10) {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
        longPressTimerRef.current = null;
      }
    }
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    touchStartRef.current = null;
  }, []);

  const closeMenu = useCallback(() => {
    setMenuPosition(null);
    setMenuItem(null);
  }, []);

  // Pin/unpin handler
  const handleTogglePin = useCallback((itemId: string) => {
    setPinnedItems(prev => {
      if (prev.includes(itemId)) {
        return prev.filter(id => id !== itemId);
      } else {
        return [...prev, itemId];
      }
    });
  }, []);

  // Delete chat handler
  const handleDeleteChat = useCallback(async (chatId: string) => {
    try {
      await deleteChat(chatId);
    } catch (err) {
      console.error('Failed to delete chat:', err);
    }
  }, [deleteChat]);

  // Leave group handler
  const handleLeaveGroup = useCallback(async (groupId: string) => {
    try {
      // Open the group first to set activeGroupId, then leave
      await openGroup(groupId);
      await leaveGroup();
    } catch (err) {
      console.error('Failed to leave group:', err);
    }
  }, [openGroup, leaveGroup]);

  // Mark as unread handler (local only)
  const handleMarkUnread = useCallback((item: UnifiedItem) => {
    // This is a local-only feature - just a visual indicator
    // In a full implementation, you'd update the store to show unread badge
    console.log('Mark as unread:', item.id);
    // For now, we'll just close the menu - full implementation would need store changes
  }, []);

  return (
    <div className="unified-messages">
      <div className="unified-toolbar">
        <ChatSearch
          value={query}
          onChange={setQuery}
          placeholder="Search Spider, apps, or chats..."
        />
      </div>

      {error && <div className="unified-error">{error}</div>}

      <div className="unified-scroll">
        <section className="unified-section">
          <div className="unified-list">
            {isLoading && unifiedItems.length === 0 ? (
              <div className="unified-empty">Loading chats…</div>
            ) : unifiedItems.length ? (
              unifiedItems.map((item) => (
                <button
                  key={item.id}
                  className={`unified-item ${item.kind} ${item.isPinned ? 'pinned' : ''}`}
                  onClick={item.onClick}
                  onContextMenu={(e) => handleContextMenu(e, item)}
                  onTouchStart={(e) => handleTouchStart(e, item)}
                  onTouchMove={handleTouchMove}
                  onTouchEnd={handleTouchEnd}
                >
                  <div className={`unified-avatar ${item.kind === 'spider' ? 'spider-avatar' : ''}`} aria-hidden="true">
                    {item.kind === 'spider' ? 'S' : item.title.slice(0, 2).toUpperCase()}
                  </div>
                  <div className="unified-item-body">
                    <div className="unified-item-row">
                      <div className="unified-item-title">
                        {item.title}
                        {item.isPinned && (
                          <span className="pin-badge" title="Pinned">
                            <span className="material-symbols-outlined" style={{ fontSize: 14 }}>push_pin</span>
                          </span>
                        )}
                      </div>
                      <div className="unified-item-meta">
                        {item.lastActivity ? (
                          <span className="unified-time">
                            {formatTime(item.lastActivity)}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="unified-item-row secondary">
                      <div className="unified-item-subtitle">
                        {item.threadPath && (
                          <span className="unified-thread-path">{item.threadPath}: </span>
                        )}
                        {item.subtitle}
                      </div>
                      {item.unread ? (
                        <span className="unified-unread">{item.unread}</span>
                      ) : null}
                    </div>
                  </div>
                </button>
              ))
            ) : (
              <div className="unified-empty">
                {query
                  ? 'No chats match your search.'
                  : 'No conversations yet. Start a DM or create a group.'}
              </div>
            )}
          </div>
        </section>
      </div>

      {/* Floating Action Button */}
      <button
        className="fab-button"
        onClick={openChooser}
        aria-label="New chat"
      >
        <span className="material-symbols-outlined">edit</span>
      </button>

      {showChooser && (
        <div className="modal-overlay" onClick={handleChooserOverlayClick}>
          <div className="modal-content chooser chooser-flow" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>New chat</h3>
              <button className="close-button" onClick={closeChooser}>
                ×
              </button>
            </div>
            <div className="chooser-flow-body">
              <div className={`chooser-panel ${chooserStep === 'chooser' ? 'active' : ''}`}>
                <div className="chooser-actions">
                  <button
                    className="chooser-action"
                    onClick={() => handleChooserStep('group')}
                  >
                    <span className="chooser-icon material-symbols-outlined" aria-hidden="true">
                      groups
                    </span>
                    <div>
                      <div className="chooser-title">Create Group Chat</div>
                      <div className="chooser-subtitle">Name it and invite members</div>
                    </div>
                  </button>
                  <button
                    className="chooser-action"
                    onClick={() => handleChooserStep('dm')}
                  >
                    <span className="chooser-icon material-symbols-outlined" aria-hidden="true">
                      chat
                    </span>
                    <div>
                      <div className="chooser-title">Start One-on-One</div>
                      <div className="chooser-subtitle">Message a single user</div>
                    </div>
                  </button>
                </div>
              </div>

              <form
                className={`chooser-panel chooser-form ${chooserStep === 'dm' ? 'active' : ''}`}
                onSubmit={handleDmSubmit}
              >
                <div className="chooser-form-fields">
                  <label>
                    <span>Node address</span>
                    <input
                      type="text"
                      placeholder="e.g., alice.os"
                      value={dmCounterparty}
                      onChange={(e) => setDmCounterparty(e.target.value)}
                      autoFocus={showChooser && chooserStep === 'dm'}
                    />
                  </label>
                </div>
                {chooserError && chooserStep === 'dm' && <div className="chooser-error">{chooserError}</div>}
                <div className="chooser-form-actions">
                  <button type="button" className="secondary" onClick={() => handleChooserStep('chooser')}>
                    Back
                  </button>
                  <button type="submit" className="primary" disabled={!dmCounterparty.trim() || isSubmittingDm}>
                    {isSubmittingDm ? 'Starting…' : 'Start Chat'}
                  </button>
                </div>
              </form>

              <form
                className={`chooser-panel chooser-form ${chooserStep === 'group' ? 'active' : ''}`}
                onSubmit={handleGroupSubmit}
              >
                <div className="chooser-form-fields">
                  <label>
                    <span>Group name</span>
                    <input
                      type="text"
                      placeholder="Team updates"
                      value={groupName}
                      onChange={(e) => setGroupName(e.target.value)}
                      autoFocus={showChooser && chooserStep === 'group'}
                    />
                  </label>
                  <label>
                    <span>Description</span>
                    <textarea
                      placeholder="What is this group for? (optional)"
                      rows={3}
                      value={groupDescription}
                      onChange={(e) => setGroupDescription(e.target.value)}
                    />
                  </label>
                  <label>
                    <span>Visibility</span>
                    <select
                      value={groupVisibility}
                      onChange={(e) =>
                        setGroupVisibility(e.target.value as unknown as api.GroupVisibility)
                      }
                    >
                      <option value={api.GroupVisibility.Private}>Private</option>
                      <option value={api.GroupVisibility.Public}>Public</option>
                    </select>
                  </label>
                </div>
                {chooserError && chooserStep === 'group' && <div className="chooser-error">{chooserError}</div>}
                <div className="chooser-form-actions">
                  <button type="button" className="secondary" onClick={() => handleChooserStep('chooser')}>
                    Back
                  </button>
                  <button type="submit" className="primary" disabled={!groupName.trim() || isSubmittingGroup}>
                    {isSubmittingGroup ? 'Creating…' : 'Create Group'}
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}

      {/* Context menu */}
      {menuPosition && menuItem && (
        <ChatItemMenu
          itemKind={menuItem.kind}
          itemTitle={menuItem.title}
          position={menuPosition}
          onClose={closeMenu}
          onDelete={menuItem.chatId ? () => handleDeleteChat(menuItem.chatId!) : undefined}
          onLeave={menuItem.groupId ? () => handleLeaveGroup(menuItem.groupId!) : undefined}
          onMarkUnread={() => handleMarkUnread(menuItem)}
          onTogglePin={() => handleTogglePin(menuItem.id)}
          isPinned={menuItem.isPinned ?? false}
        />
      )}
    </div>
  );
};

export default UnifiedMessages;
