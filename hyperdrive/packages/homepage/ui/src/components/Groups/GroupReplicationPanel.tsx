import React, { useEffect } from 'react';
import { Chat } from '#caller-utils';
import './GroupReplicationPanel.css';

interface GroupReplicationPanelProps {
  replication?: Chat.GroupReplicationState;
  whitelist?: Chat.AdminWhitelistRes | null;
}

const formatCursor = (cursor: Chat.DeliveryCursor | undefined) => {
  if (!cursor) return '—';
  return `q:${cursor.queue_id} • offset ${cursor.last_offset}`;
};

const valuesOf = <T,>(input: unknown): T[] => {
  if (!input) return [];
  if (Array.isArray(input)) return input as T[];
  if (input instanceof Map) return Array.from(input.values()) as T[];
  if (typeof input === 'object') return Object.values(input as Record<string, T>);
  return [];
};

const entriesOf = <T,>(input: unknown): [string, T][] => {
  if (!input) return [];
  if (Array.isArray(input)) return input as [string, T][];
  if (input instanceof Map) return Array.from(input.entries()) as [string, T][];
  if (typeof input === 'object') return Object.entries(input as Record<string, T>);
  return [];
};

const GroupReplicationPanel: React.FC<GroupReplicationPanelProps> = ({
  replication,
  whitelist,
}) => {
  if (!replication) {
    console.warn('[GROUP REPL] No replication info loaded yet.');
    return null;
  }

  const hubs: string[] = valuesOf<string>(replication.hubs);
  const subscribers: string[] = valuesOf<string>(replication.subscribers);
  const hubCount = hubs.length;
  const subscriberCount = subscribers.length;
  const whitelistEntries: Chat.WhitelistEntryDebug[] = valuesOf<Chat.WhitelistEntryDebug>(
    whitelist?.entries,
  );
  const hubCursors: [string, Chat.DeliveryCursor][] = entriesOf<Chat.DeliveryCursor>(
    replication.hub_cursors,
  );
  const subscriberCursors: [string, Chat.DeliveryCursor][] = entriesOf<Chat.DeliveryCursor>(
    replication.subscriber_cursors,
  );
  const subscriberLag = replication.subscriber_lag_secs;
  if (subscriberLag && subscriberLag > 0) {
    console.log(`[GROUP REPL] Subscriber lag for ${replication.group_id ?? 'group'}: ${subscriberLag}s`);
  }

  useEffect(() => {
    const hubCursorSummary = hubCursors.map(([node, cursor]) => ({
      node,
      cursor: formatCursor(cursor),
    }));
    const subscriberCursorSummary = subscriberCursors.map(([node, cursor]) => ({
      node,
      cursor: formatCursor(cursor),
    }));
    const whitelistSummary = {
      version: whitelist?.version ?? null,
      entries: whitelistEntries.length,
    };
    console.log('[GROUP REPL]', {
      group: (replication as any)?.group_id ?? 'unknown',
      hubs: hubCount,
      subscribers: subscriberCount,
      hubLag: replication.hub_lag_secs ?? null,
      subscriberLag: replication.subscriber_lag_secs ?? null,
      hubCursors: hubCursorSummary,
      subscriberCursors: subscriberCursorSummary,
      whitelist: whitelistSummary,
    });
  }, [
    hubCount,
    subscriberCount,
    replication.hub_lag_secs,
    replication.subscriber_lag_secs,
    hubCursors,
    subscriberCursors,
    whitelistEntries.length,
    whitelist?.version,
  ]);

  return null;
};

export default GroupReplicationPanel;
