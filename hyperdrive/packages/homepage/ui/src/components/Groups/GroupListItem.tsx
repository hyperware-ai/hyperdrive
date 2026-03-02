import React from 'react';
import { Chat } from '#caller-utils';
import './GroupListItem.css';

interface GroupListItemProps {
  summary: Chat.GroupSummary;
  onSelect: (groupId: string) => void;
  replicationState?: Chat.GroupReplicationState;
  isActive?: boolean;
  index?: number;
}

const formatTimestamp = (timestamp?: number | null) => {
  if (!timestamp) return 'No activity yet';
  const date = new Date(timestamp * 1000);
  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  if (diffHours < 1) {
    const mins = Math.max(1, Math.floor(diffMs / (1000 * 60)));
    return `${mins}m ago`;
  }
  if (diffHours < 24) return `${diffHours}h ago`;
  return date.toLocaleDateString();
};

const GroupListItem: React.FC<GroupListItemProps> = ({
  summary,
  replicationState,
  onSelect,
  isActive,
  index = 0,
}) => {
  const name = summary.metadata?.name || 'Untitled group';
  const description = summary.metadata?.description || 'No description';
  const updatedAt = summary.metadata?.updated_at;

  const status = (() => {
    if (!replicationState) return { label: 'Pending', tone: 'muted' as const };
    if (replicationState.pending_bootstrap) {
      return { label: 'Bootstrapping', tone: 'warn' as const };
    }
    if (
      replicationState.subscriber_lag_secs &&
      replicationState.subscriber_lag_secs > 45
    ) {
      console.warn(
        `[GROUP REPL] Subscriber lag for ${summary.group_id}: ${replicationState.subscriber_lag_secs}s`,
      );
    }
    return { label: 'Live', tone: 'ok' as const };
  })();

  return (
    <button
      className={`group-list-item ${isActive ? 'active' : ''}`}
      onClick={() => onSelect(summary.group_id)}
      style={{ '--item-index': index } as React.CSSProperties}
    >
      <div className="group-list-title-row">
        <div className="group-avatar">
          <span>{name.slice(0, 2).toUpperCase()}</span>
        </div>
        <div className="group-title">
          <div className="group-name">{name}</div>
          <div className="group-meta">
            <span>{summary.member_count} members</span>
            <span className="dot">•</span>
            <span>{summary.thread_count} threads</span>
          </div>
        </div>
        <div className={`group-status group-status-${status.tone}`}>
          {status.label}
        </div>
      </div>
      <div className="group-description">{description}</div>
      <div className="group-footer">
        <span className="group-updated">Updated {formatTimestamp(updatedAt)}</span>
      </div>
    </button>
  );
};

export default GroupListItem;
