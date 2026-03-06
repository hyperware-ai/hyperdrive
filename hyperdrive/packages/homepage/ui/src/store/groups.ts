import { create } from 'zustand';
import { Chat } from '#caller-utils';
import { GroupMessage, NormalizedGroup } from '../types/groups';
import { useChatStore } from './chat';

type BodyCache = Record<string, string>;
type GroupPreview = {
  text: string;
  timestamp: number | null;
  threadPath?: string | null; // e.g. "Main > Feature Discussion" if message is in a non-root thread
};

const BODY_CACHE_KEY = 'group-message-bodies';
const BODY_CACHE_LIMIT = 400;

function loadBodyCache(): BodyCache {
  try {
    const raw = localStorage.getItem(BODY_CACHE_KEY);
    return raw ? (JSON.parse(raw) as BodyCache) : {};
  } catch (error) {
    console.warn('[GROUPS] Failed to load body cache', error);
    return {};
  }
}

function persistBodyCache(bodies: BodyCache): BodyCache {
  // Keep only the most recent entries to avoid unbounded growth.
  const entries = Object.entries(bodies);
  const trimmed =
    entries.length > BODY_CACHE_LIMIT
      ? Object.fromEntries(entries.slice(entries.length - BODY_CACHE_LIMIT))
      : bodies;
  try {
    localStorage.setItem(BODY_CACHE_KEY, JSON.stringify(trimmed));
  } catch (error) {
    console.warn('[GROUPS] Failed to persist body cache', error);
  }
  return trimmed;
}

function toFlexibleMap<T>(
  input:
    | [string, T][]
    | Record<string, T>
    | Map<string, T>
    | T[]
    | null
    | undefined,
  keySelector: (value: T) => string | null | undefined,
): Map<string, T> {
  const map = new Map<string, T>();
  if (!input) return map;
  if (input instanceof Map) {
    return new Map(input);
  }
  if (Array.isArray(input)) {
    for (const entry of input) {
      if (Array.isArray(entry) && entry.length >= 2) {
        map.set(String(entry[0]), entry[1] as T);
      } else {
        const key = keySelector(entry as T);
        if (key) {
          map.set(String(key), entry as T);
        }
      }
    }
    return map;
  }
  Object.entries(input).forEach(([k, v]) => {
    map.set(k, v as T);
  });
  return map;
}

function ensureMetadata(
  groupId: string,
  metadata?: Chat.GroupMetadata | null,
): Chat.GroupMetadata {
  const now = Math.floor(Date.now() / 1000);
  return (
    metadata ?? {
      name: 'New Group',
      description: null,
      avatar: null,
      creator_id: '',
      created_at: now,
      updated_at: now,
      visibility: Chat.GroupVisibility.Private,
      default_role_id: `${groupId}:member`,
      root_thread_id: `${groupId}:thread:root`,
    }
  );
}

function describeAttachments(
  attachments: Chat.AttachmentDescriptor[] | undefined,
): string | null {
  if (!attachments || attachments.length === 0) return null;
  const [first] = attachments;
  if (first.filename) return first.filename;
  if (first.mime_type) return first.mime_type;
  return 'Attachment';
}

function buildMessagePreview(
  meta: Chat.MessageMeta | undefined,
  bodyCache: BodyCache,
): { text: string; timestamp: number | null; cache: BodyCache } {
  if (!meta) {
    return { text: 'No messages yet', timestamp: null, cache: bodyCache };
  }

  const cache = { ...bodyCache };
  const metaBody = (meta as any).body as string | undefined;
  if (metaBody?.trim?.()) {
    cache[meta.message_id] = metaBody.trim();
  }
  const text =
    metaBody?.trim?.() ||
    cache[meta.message_id] ||
    describeAttachments(meta.attachments) ||
    'Message payload unavailable';

  const timestamp = meta.timestamp ?? null;
  return { text, timestamp, cache };
}

function extractLatestMessagePreview(
  group: Chat.Group,
  bodyCache: BodyCache,
): { text: string; timestamp: number | null; threadPath: string | null; cache: BodyCache } {
  const messagesRaw = toFlexibleMap<Chat.MessageMeta>(
    group.messages as any,
    (m) => (m as any)?.message_id,
  );
  const messages = Array.from(messagesRaw.values()).sort(
    (a, b) => (a.timestamp ?? 0) - (b.timestamp ?? 0),
  );
  const latest = messages[messages.length - 1];
  const { text, timestamp, cache } = buildMessagePreview(latest, bodyCache);

  // Build thread path if message is not in root thread
  let threadPath: string | null = null;
  if (latest?.thread_id) {
    const threads = toFlexibleMap<Chat.Thread>(group.threads as any, (t) => (t as any)?.id);
    const thread = threads.get(latest.thread_id);
    if (thread && thread.depth > 0) {
      // Build path from current thread up, excluding root (Main) thread
      const path: string[] = [];
      let current: Chat.Thread | undefined = thread;
      while (current && current.depth > 0) {
        const title = current.title || 'Thread';
        path.unshift(title);
        const parentRef = current.parent as any;
        if (parentRef && 'Thread' in parentRef) {
          current = threads.get(parentRef.Thread as string);
        } else {
          break;
        }
      }
      if (path.length > 0) {
        threadPath = path.join(' › ');
      }
    }
  }

  return {
    text,
    timestamp: timestamp ?? group.metadata?.updated_at ?? null,
    threadPath,
    cache,
  };
}

function normalizeGroup(
  groupId: string,
  group: Chat.Group,
  bodyCache: BodyCache,
): NormalizedGroup {
  const metadata = ensureMetadata(groupId, group.metadata);
  const roles = toFlexibleMap<Chat.Role>(group.roles as any, (role) => role?.id);
  const members = toFlexibleMap<Chat.GroupMember>(group.members as any, (m) => (m as any)?.node_id);
  const threads = toFlexibleMap<Chat.Thread>(group.threads as any, (t) => (t as any)?.id);
  const messagesRaw = toFlexibleMap<Chat.MessageMeta>(
    group.messages as any,
    (m) => (m as any)?.message_id,
  );
  const proposalsRaw = toFlexibleMap<Chat.MembershipProposal>(
    group.membership_proposals as any,
    (p) => (p as any)?.proposal_id,
  );

  const messages: GroupMessage[] = Array.from(messagesRaw.values()).map(
    (meta) => {
      const metaBody = (meta as any).body as string | undefined;
      const content =
        metaBody?.trim?.() ||
        bodyCache[meta.message_id] ||
        describeAttachments(meta.attachments) ||
        'Message payload unavailable';

      return {
        id: meta.message_id,
        threadId: meta.thread_id,
        sender: meta.sender,
        timestamp: meta.timestamp,
        type: meta.message_type,
        replyTo: meta.reply_to ?? null,
        attachments: meta.attachments ?? [],
        content,
        status: 'delivered',
        reactions:
          (meta as any).reactions?.map((r: any) => ({
            emoji: r.emoji,
            user: r.node_id,
            timestamp: r.timestamp,
          })) ?? [],
      };
    },
  );

  messages.sort((a, b) => a.timestamp - b.timestamp);

  const rootThreadId =
    metadata.root_thread_id ||
    (threads.size > 0 ? Array.from(threads.keys())[0] : null);
  const proposals = Array.from(proposalsRaw.values());

  const membershipRules = (group.membership_rules as Chat.MembershipRuleConfig[]) || [];

  return {
    id: groupId,
    metadata,
    roles,
    members,
    threads,
    messages,
    rootThreadId,
    proposals,
    membershipRules,
  };
}

interface DraftThread {
  parentThreadId: string;
  rootMessageId: string | null;
  title: string | null;
}

interface GroupStore {
  groups: Chat.GroupSummary[];
  groupPreviews: Record<string, GroupPreview>;
  groupUnread: Record<string, number>;
  groupNotify: Record<string, boolean>;
  activeGroupId: string | null;
  activeGroup: NormalizedGroup | null;
  activeThreadId: string | null;
  draftThread: DraftThread | null;
  replyingTo: GroupMessage | null;
  editingMessage: { id: string; content: string } | null;
  subscriberEvents: Chat.SubscriberDeliveryEvent[];
  replication: Record<string, Chat.GroupReplicationState>;
  replicationMetrics: Chat.ReplicationMetrics | null;
  whitelists: Record<string, Chat.AdminWhitelistRes | null>;
  isLoading: boolean;
  isSyncing: boolean;
  error: string | null;
  messageBodies: BodyCache;
  jumpToMessageId: string | null;
  loadGroups: () => Promise<void>;
  openGroup: (groupId: string) => Promise<void>;
  refreshActiveGroup: () => Promise<void>;
  createGroup: (input: {
    name: string;
    description?: string;
    visibility?: Chat.GroupVisibility;
    rootThreadTitle?: string | null;
  }) => Promise<string | null>;
  setActiveThread: (threadId: string) => void;
  clearActiveGroup: () => void;
  setReplyingTo: (message: GroupMessage | null) => void;
  setEditingMessage: (message: { id: string; content: string } | null) => void;
  startDraftThread: (
    parentThreadId: string,
    rootMessageId?: string | null,
  ) => void;
  clearDraftThread: () => void;
  createThread: (
    title: string | null,
    parentThreadId?: string | null,
    rootMessageId?: string | null,
  ) => Promise<string | null>;
  sendMessage: (content: string, replyTo?: string | null) => Promise<void>;
  editMessage: (messageId: string, newContent: string) => Promise<void>;
  deleteMessage: (messageId: string) => Promise<void>;
  forwardMessage: (messageId: string, toGroupId: string) => Promise<void>;
  addReaction: (messageId: string, emoji: string) => Promise<void>;
  removeReaction: (messageId: string, emoji: string) => Promise<void>;
  fetchSubscriberEvents: (clear?: boolean) => Promise<void>;
  fetchReplicationState: (groupId?: string | null) => Promise<void>;
  fetchWhitelist: (groupId?: string | null) => Promise<void>;
  inviteMember: (candidate: string, roleId: string) => Promise<Chat.MembershipDecision | null>;
  approveProposal: (proposalId: string) => Promise<Chat.MembershipDecision | null>;
  removeMember: (member: string) => Promise<Chat.MembershipDecision | null>;
  leaveGroup: () => Promise<Chat.MembershipDecision | null>;
  refreshGroupPreviews: (groupIds?: string[]) => Promise<void>;
  toggleReaction: (messageId: string, emoji: string) => Promise<void>;
  markGroupAsRead: (groupId: string) => void;
  updateGroupSettings: (groupId: string, settings: { notify?: boolean }) => Promise<void>;
  createGroupJoinLink: (groupId: string) => Promise<string | null>;
  joinGroupLink: (host: string, key: string) => Promise<string | null>;
  setJumpToMessageId: (messageId: string | null) => void;
}

export const useGroupStore = create<GroupStore>((set, get) => ({
  groups: [],
  groupPreviews: {},
  groupUnread: {},
  groupNotify: {},
  activeGroupId: null,
  activeGroup: null,
  activeThreadId: null,
  draftThread: null,
  replyingTo: null,
  editingMessage: null,
  subscriberEvents: [],
  replication: {},
  replicationMetrics: null,
  whitelists: {},
  isLoading: false,
  isSyncing: false,
  error: null,
  messageBodies: loadBodyCache(),
  jumpToMessageId: null,

  loadGroups: async () => {
    try {
      set({ isLoading: true });
      const res = await Chat.list_groups();
      const groups = (Array.isArray(res.groups)
        ? res.groups
        : Object.values(res.groups || {})) as Chat.GroupSummary[];
      const sorted = [...groups].sort((a, b) => {
        const aTs = a.metadata?.updated_at ?? 0;
        const bTs = b.metadata?.updated_at ?? 0;
        return bTs - aTs;
      });
      // Build groupUnread and groupNotify maps from API response
      const groupUnread: Record<string, number> = {};
      const groupNotify: Record<string, boolean> = {};
      for (const g of sorted) {
        if ((g as any).unread_count !== undefined) {
          groupUnread[g.group_id] = (g as any).unread_count;
        }
        // Default to true if not specified
        groupNotify[g.group_id] = (g as any).notify ?? true;
      }
      set({ groups: sorted, groupUnread, groupNotify, error: null });
      await get().refreshGroupPreviews(sorted.map((g) => g.group_id));
    } catch (error) {
      console.error('[GROUPS] Failed to load groups', error);
      set({ error: 'Failed to load groups' });
    } finally {
      set({ isLoading: false });
    }
  },

  openGroup: async (groupId: string) => {
    try {
      set({ isLoading: true, activeGroupId: groupId });
      const res = await Chat.get_group({ group_id: groupId });
      if (!res.group) {
        set({ error: 'Group not found', activeGroup: null, activeThreadId: null });
        return;
      }

      let normalized: NormalizedGroup;
      try {
        normalized = normalizeGroup(groupId, res.group, get().messageBodies);
      } catch (err) {
        console.error('[GROUPS] Failed to normalize group', err);
        set({ error: 'Unable to load group', activeGroup: null, activeThreadId: null });
        return;
      }
      const activeThreadId =
        normalized.rootThreadId ||
        normalized.messages[normalized.messages.length - 1]?.threadId ||
        null;

      set((state) => {
        const lastMessage = normalized.messages[normalized.messages.length - 1];
        const previewText = lastMessage?.content || 'No messages yet';
        const previewTimestamp = lastMessage?.timestamp ?? normalized.metadata.updated_at ?? null;
        const updatedBodies =
          lastMessage?.id && previewText
            ? persistBodyCache({
                ...state.messageBodies,
                [lastMessage.id]: previewText,
              })
            : state.messageBodies;
        return {
          activeGroup: normalized,
          activeThreadId,
          error: null,
          messageBodies: updatedBodies,
          groupPreviews: {
            ...state.groupPreviews,
            [groupId]: { text: previewText, timestamp: previewTimestamp },
          },
        };
      });
      // Refresh delivery/replication state alongside opening the group.
      get().fetchReplicationState(groupId);
      get().fetchSubscriberEvents();
    } catch (error) {
      console.error('[GROUPS] Failed to open group', error);
      // If access denied (e.g., kicked from group), clear active state
      const errMsg = String(error);
      if (
        errMsg.includes('lacks subscribe access') ||
        errMsg.includes('cannot get group') ||
        errMsg.includes('not a member')
      ) {
        console.log('[GROUPS] Access denied to group, clearing state');
        set({
          error: 'You no longer have access to this group',
          activeGroup: null,
          activeGroupId: null,
          activeThreadId: null,
        });
      } else {
        set({ error: 'Failed to load group', activeGroup: null, activeThreadId: null });
      }
    } finally {
      set({ isLoading: false });
    }
  },

  refreshActiveGroup: async () => {
    const groupId = get().activeGroupId;
    if (!groupId) return;
    try {
      set({ isSyncing: true });
      const res = await Chat.get_group({ group_id: groupId });
      if (!res.group) return;

      // Debug: log raw messages to check for reactions
      const rawMessages = (res.group as any).messages;
      console.log('[GROUPS DEBUG] Raw messages from get_group:', JSON.stringify(rawMessages, null, 2));

      let normalized: NormalizedGroup;
      try {
        normalized = normalizeGroup(groupId, res.group, get().messageBodies);
      } catch (err) {
        console.error('[GROUPS] Failed to normalize group on refresh', err);
        set({ error: 'Failed to refresh group', isSyncing: false });
        return;
      }
      set((state) => {
        const lastMessage = normalized.messages[normalized.messages.length - 1];
        const previewText = lastMessage?.content || 'No messages yet';
        const previewTimestamp = lastMessage?.timestamp ?? normalized.metadata.updated_at ?? null;
        const updatedBodies =
          lastMessage?.id && previewText
            ? persistBodyCache({
                ...state.messageBodies,
                [lastMessage.id]: previewText,
              })
            : state.messageBodies;
        // Preserve activeThreadId if set, only fall back to root if it's null
        // This prevents race conditions when a thread was just created
        // Also preserve null if we're in draft mode (draftThread exists)
        const newActiveThreadId = state.draftThread
          ? state.activeThreadId
          : state.activeThreadId || normalized.rootThreadId;
        return {
          activeGroup: normalized,
          activeThreadId: newActiveThreadId,
          messageBodies: updatedBodies,
          groupPreviews: {
            ...state.groupPreviews,
            [groupId]: { text: previewText, timestamp: previewTimestamp },
          },
        };
      });
      get().fetchReplicationState(groupId);
    } catch (error) {
      console.error('[GROUPS] Failed to refresh group', error);
      // If we lost access (e.g., kicked from group), clear the active group
      const errMsg = String(error);
      if (
        errMsg.includes('lacks subscribe access') ||
        errMsg.includes('cannot get group') ||
        errMsg.includes('not a member')
      ) {
        console.log('[GROUPS] Access denied, clearing active group');
        get().clearActiveGroup();
      } else {
        set({ error: 'Failed to refresh group' });
      }
    } finally {
      set({ isSyncing: false });
    }
  },

  createGroup: async (input) => {
    try {
      set({ isLoading: true });
      const payload: Chat.CreateGroupReq = {
        group_id: null,
        name: input.name,
        description: input.description ?? null,
        avatar: null,
        visibility: input.visibility ?? Chat.GroupVisibility.Private,
        default_role_label: null,
        membership_rules: [],
        root_thread_title: input.rootThreadTitle ?? null,
      };

      const res = await Chat.create_group(payload);
      await get().loadGroups();
      await get().openGroup(res.group_id);
      return res.group_id;
    } catch (error) {
      console.error('[GROUPS] Failed to create group', error);
      set({ error: 'Failed to create group' });
      return null;
    } finally {
      set({ isLoading: false });
    }
  },

  setActiveThread: (threadId: string) => set({ activeThreadId: threadId, draftThread: null }),

  clearActiveGroup: () =>
    set({
      activeGroup: null,
      activeGroupId: null,
      activeThreadId: null,
      draftThread: null,
      replyingTo: null,
      isSyncing: false,
      jumpToMessageId: null,
    }),

  setJumpToMessageId: (messageId: string | null) => set({ jumpToMessageId: messageId }),

  setReplyingTo: (message) => set({ replyingTo: message }),

  setEditingMessage: (message) => set({ editingMessage: message }),

  startDraftThread: (parentThreadId: string, rootMessageId: string | null = null) => {
    const activeGroup = get().activeGroup;
    if (!activeGroup) return;

    // Generate title from root message if available
    let title: string | null = null;
    if (rootMessageId) {
      const rootMessage = activeGroup.messages.find((m) => m.id === rootMessageId);
      if (rootMessage) {
        const maxLen = 50;
        title = rootMessage.content.length > maxLen
          ? rootMessage.content.substring(0, maxLen).trim() + '…'
          : rootMessage.content.trim();
      }
    }

    set({
      draftThread: { parentThreadId, rootMessageId, title },
      activeThreadId: null, // Clear active thread to show draft view
    });
  },

  clearDraftThread: () => set({ draftThread: null }),

  createThread: async (title, parentThreadIdOverride = null, rootMessageId = null) => {
    const groupId = get().activeGroupId;
    const parentThreadId =
      parentThreadIdOverride !== null && parentThreadIdOverride !== undefined
        ? parentThreadIdOverride
        : get().activeGroup?.rootThreadId ?? null;
    if (!groupId) return null;
    try {
      // rootMessageId is captured here so we can send it once the backend accepts it.
      if (rootMessageId) {
        console.log('[GROUPS] Start sub-thread from message', {
          groupId,
          parentThreadId,
          rootMessageId,
        });
      }

      const res = await Chat.create_group_thread({
        group_id: groupId,
        parent_thread_id: parentThreadId,
        title: title || null,
        root_message_id: rootMessageId ?? null,
      });
      await get().refreshActiveGroup();
      // Set the active thread AFTER refreshing to ensure it sticks
      set({ activeThreadId: res.thread_id });
      return res.thread_id;
    } catch (error) {
      console.error('[GROUPS] Failed to create thread', error);
      set({ error: 'Unable to create thread' });
      return null;
    }
  },

  sendMessage: async (content: string, replyTo: string | null = null) => {
    const groupId = get().activeGroupId;
    let threadId = get().activeThreadId;
    const draftThread = get().draftThread;
    const sender = (window as any).our?.node || 'me';

    // If we have a draft thread, handle thread creation
    if (draftThread && !threadId) {
      console.log('[GROUPS] Creating thread from draft');

      const parentThreadId = draftThread.parentThreadId;

      // If no root message exists (new thread from menu), the user's message becomes the root
      if (!draftThread.rootMessageId && parentThreadId) {
        try {
          // Send the message to the parent thread - this becomes the root message
          const rootMsgRes = await Chat.send_group_message({
            group_id: groupId!,
            thread_id: parentThreadId,
            content,
            message_type: Chat.MessageType.Text,
            reply_to: replyTo,
            attachments: [],
          });
          const rootMessageId = rootMsgRes.message.message_id;

          // Use the message content as thread title
          const threadTitle = content.length > 50 ? content.substring(0, 50).trim() + '…' : content.trim();

          // Create thread with this message as root
          const newThreadId = await get().createThread(
            threadTitle,
            parentThreadId,
            rootMessageId,
          );
          if (!newThreadId) {
            console.error('[GROUPS] Failed to create thread from draft');
            return;
          }

          set({ draftThread: null, activeThreadId: newThreadId });
          // Don't send another message - the root message IS the user's message
          return;
        } catch (error) {
          console.error('[GROUPS] Failed to create thread from new message', error);
          return;
        }
      }

      // Starting thread from existing message - create thread and send first reply
      const threadTitle = draftThread.title || (content.length > 50 ? content.substring(0, 50).trim() + '…' : content.trim());
      const newThreadId = await get().createThread(
        threadTitle,
        parentThreadId,
        draftThread.rootMessageId,
      );
      if (!newThreadId) {
        console.error('[GROUPS] Failed to create thread from draft');
        return;
      }
      threadId = newThreadId;
      set({ draftThread: null, activeThreadId: newThreadId });
    }

    if (!groupId || !threadId) return;

    const timestamp = Math.floor(Date.now() / 1000);
    const tempId = `temp-${timestamp}-${Math.random().toString(16).slice(2)}`;
    const optimistic: GroupMessage = {
      id: tempId,
      threadId,
      sender,
      timestamp,
      type: Chat.MessageType.Text,
      replyTo,
      attachments: [],
      content,
      status: 'sending' as const,
      isLocal: true,
    };

    set((state) => {
      if (!state.activeGroup) return state;
      const updatedBodies = persistBodyCache({
        ...state.messageBodies,
        [tempId]: content,
      });
      return {
        activeGroup: {
          ...state.activeGroup,
          messages: [...state.activeGroup.messages, optimistic],
        },
        messageBodies: updatedBodies,
        error: null,
        groupPreviews: {
          ...state.groupPreviews,
          [groupId]: { text: content, timestamp },
        },
      };
    });

    try {
      const res = await Chat.send_group_message({
        group_id: groupId,
        thread_id: threadId,
        content,
        message_type: Chat.MessageType.Text,
        reply_to: replyTo,
        attachments: [],
      });
      const realId = res.message.message_id;
      set((state) => {
        if (!state.activeGroup) return state;
        const updatedBodies = persistBodyCache({
          ...state.messageBodies,
          [realId]: content,
        });
        const messages = state.activeGroup.messages.map((msg) =>
          msg.id === tempId
            ? { ...msg, id: realId, status: 'sent' as const }
            : msg,
        );
        return {
          activeGroup: { ...state.activeGroup, messages },
          messageBodies: updatedBodies,
          groupPreviews: {
            ...state.groupPreviews,
            [groupId]: { text: content, timestamp },
          },
        };
      });
      // Pull a fresh copy so we pick up any server-side mutations.
      get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to send message', error);
      set((state) => {
        if (!state.activeGroup) return state;
        return {
          activeGroup: {
            ...state.activeGroup,
            messages: state.activeGroup.messages.map((msg) =>
              msg.id === tempId ? { ...msg, status: 'failed' as const } : msg,
            ),
          },
          error: 'Failed to send message',
        };
      });
    }
  },

  toggleReaction: async (messageId: string, emoji: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;
    const ourNode = (window as any).our?.node || 'me';

    // Check if already reacted BEFORE optimistic update
    const msg = get().activeGroup?.messages.find((m) => m.id === messageId);
    const hasReacted = msg?.reactions?.some((r) => r.user === ourNode && r.emoji === emoji);

    // Optimistic update
    set((state) => {
      if (!state.activeGroup) return state;
      const updatedMessages = state.activeGroup.messages.map((m) => {
        if (m.id !== messageId) return m;
        const existing = m.reactions || [];
        const alreadyReacted = existing.some((r) => r.user === ourNode && r.emoji === emoji);
        const reactions = alreadyReacted
          ? existing.filter((r) => !(r.user === ourNode && r.emoji === emoji))
          : [...existing, { emoji, user: ourNode, timestamp: Math.floor(Date.now() / 1000) }];
        return { ...m, reactions };
      });
      return {
        ...state,
        activeGroup: { ...state.activeGroup, messages: updatedMessages },
      };
    });

    try {
      if (hasReacted) {
        await Chat.remove_group_reaction({
          group_id: groupId,
          message_id: messageId,
          emoji,
        });
      } else {
        await Chat.add_group_reaction({
          group_id: groupId,
          message_id: messageId,
          emoji,
        });
      }
    } catch (error) {
      console.error('[GROUPS] Failed to toggle reaction', error);
      get().refreshActiveGroup();
    }
  },

  editMessage: async (messageId: string, newContent: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;

    try {
      await Chat.edit_group_message({
        group_id: groupId,
        message_id: messageId,
        new_content: newContent,
      });
      await get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to edit message', error);
      set({ error: 'Failed to edit message' });
    }
  },

  deleteMessage: async (messageId: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;

    try {
      await Chat.delete_group_message({
        group_id: groupId,
        message_id: messageId,
        delete_for_both: true,
      });
      await get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to delete message', error);
      set({ error: 'Failed to delete message' });
    }
  },

  forwardMessage: async (messageId: string, toGroupId: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;

    try {
      await Chat.forward_message({
        from_chat_id: groupId,
        message_id: messageId,
        to_chat_id: toGroupId,
      });
      // Refresh both the current group and potentially the target group
      await get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to forward message', error);
      set({ error: 'Failed to forward message' });
    }
  },

  addReaction: async (messageId: string, emoji: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;

    try {
      await Chat.add_group_reaction({
        group_id: groupId,
        message_id: messageId,
        emoji,
      });
      await get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to add reaction', error);
      set({ error: 'Failed to add reaction' });
    }
  },

  removeReaction: async (messageId: string, emoji: string) => {
    const groupId = get().activeGroupId;
    if (!groupId) return;

    try {
      await Chat.remove_group_reaction({
        group_id: groupId,
        message_id: messageId,
        emoji,
      });
      await get().refreshActiveGroup();
    } catch (error) {
      console.error('[GROUPS] Failed to remove reaction', error);
      set({ error: 'Failed to remove reaction' });
    }
  },

  fetchSubscriberEvents: async (clear = false) => {
    try {
      const res = await Chat.admin_subscriber_events({
        clear,
        take: 50,
      });
      set({ subscriberEvents: res.events });
    } catch (error) {
      console.error('[GROUPS] Failed to fetch subscriber events', error);
    }
  },

  fetchReplicationState: async (groupId: string | null = null) => {
    try {
      const res = await Chat.admin_replication_state({ group_id: groupId });
      const replication = { ...get().replication };
      const groups = (Array.isArray(res.groups)
        ? res.groups
        : Object.values(res.groups || {})) as Chat.GroupReplicationState[];
      groups.forEach((g) => {
        replication[g.group_id] = g;
      });
      set({
        replication,
        replicationMetrics: res.metrics,
      });
    } catch (error) {
      console.error('[GROUPS] Failed to fetch replication state', error);
    }
  },

  refreshGroupPreviews: async (groupIds) => {
    const targets = groupIds ?? get().groups.map((g) => g.group_id);
    if (!targets.length) return;

    let cache = { ...get().messageBodies };
    const previews = { ...get().groupPreviews };

    for (const groupId of targets) {
      try {
        const res = await Chat.get_group({ group_id: groupId });
        if (!res.group) continue;
        const { text, timestamp, threadPath, cache: updatedCache } = extractLatestMessagePreview(
          res.group,
          cache,
        );
        cache = updatedCache;
        previews[groupId] = { text, timestamp, threadPath };
      } catch (error) {
        console.error('[GROUPS] Failed to refresh preview for group', groupId, error);
      }
    }

    set({
      groupPreviews: previews,
      messageBodies: persistBodyCache(cache),
    });
  },

  fetchWhitelist: async (groupId: string | null = null) => {
    const id = groupId ?? get().activeGroupId;
    if (!id) return;
    try {
      const res = await Chat.admin_whitelist({ group_id: id });
      set((state) => ({
        whitelists: { ...state.whitelists, [id]: res },
      }));
    } catch (error) {
      console.error('[GROUPS] Failed to fetch whitelist', error);
    }
  },

  inviteMember: async (candidate, roleId) => {
    const groupId = get().activeGroupId;
    if (!groupId) return null;
    try {
      const res = await Chat.invite_group_member({
        group_id: groupId,
        candidate,
        role_id: roleId,
      });
      await get().refreshActiveGroup();
      return res.decision;
    } catch (error) {
      console.error('[GROUPS] Failed to invite member', error);
      set({ error: 'Failed to invite member' });
      return null;
    }
  },

  approveProposal: async (proposalId) => {
    const groupId = get().activeGroupId;
    if (!groupId) return null;
    try {
      const res = await Chat.approve_group_membership({
        group_id: groupId,
        proposal_id: proposalId,
      });
      await get().refreshActiveGroup();
      return res.decision;
    } catch (error) {
      console.error('[GROUPS] Failed to approve membership', error);
      set({ error: 'Failed to approve membership' });
      return null;
    }
  },

  removeMember: async (member) => {
    const groupId = get().activeGroupId;
    if (!groupId) return null;
    try {
      const res = await Chat.remove_group_member({
        group_id: groupId,
        member,
      });
      await get().refreshActiveGroup();
      return res.decision;
    } catch (error) {
      console.error('[GROUPS] Failed to remove member', error);
      set({ error: 'Failed to remove member' });
      return null;
    }
  },

  leaveGroup: async () => {
    const groupId = get().activeGroupId;
    const me = (window as any).our?.node || null;
    if (!groupId || !me) return null;

    const decision = await get().removeMember(me);
    if (decision) {
      // Remove the group from the local state after successfully leaving
      set((state) => ({
        groups: state.groups.filter((g) => g.group_id !== groupId),
        activeGroup: null,
        activeGroupId: null,
        activeThreadId: null,
        draftThread: null,
        replyingTo: null,
      }));
    }
    return decision;
  },

  markGroupAsRead: (groupId: string) => {
    // Optimistically update local state
    set((state) => ({
      groupUnread: { ...state.groupUnread, [groupId]: 0 },
    }));

    // Send via WebSocket if connected
    const ws = useChatStore.getState().wsConnection;
    if (ws) {
      ws.send({ MarkGroupRead: { group_id: groupId } });
    }
  },

  updateGroupSettings: async (groupId: string, settings: { notify?: boolean }) => {
    try {
      const res = await Chat.update_group_settings({
        group_id: groupId,
        notify: settings.notify ?? null,
      });

      // Update local state with the response
      set((state) => ({
        groupNotify: { ...state.groupNotify, [groupId]: res.notify },
      }));
    } catch (error) {
      console.error('[GROUPS] Failed to update group settings', error);
    }
  },

  createGroupJoinLink: async (groupId: string) => {
    try {
      const res = await Chat.create_group_join_link({ group_id: groupId });
      return res.link;
    } catch (error) {
      console.error('[GROUPS] Failed to create join link', error);
      set({ error: 'Failed to create join link' });
      return null;
    }
  },

  joinGroupLink: async (host: string, key: string) => {
    try {
      set({ isLoading: true });
      const res = await Chat.join_group_link({ host, key });
      await get().loadGroups();
      await get().openGroup(res.group_id);
      return res.group_id;
    } catch (error) {
      console.error('[GROUPS] Failed to join group', error);
      set({ error: 'Failed to join group' });
      return null;
    } finally {
      set({ isLoading: false });
    }
  },
}));
