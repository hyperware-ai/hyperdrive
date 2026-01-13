import React, { useState, useRef, useEffect } from 'react';
import { Chat } from '#caller-utils';
import './ThreadsDropdown.css';

type ThreadWithId = Chat.Thread & { id: string };

interface ThreadsDropdownProps {
  threads: ThreadWithId[];
  activeThreadId: string | null;
  rootThreadId: string | null;
  onSelect: (threadId: string) => void;
  onClose: () => void;
  canCreateThread: boolean;
  onCreateThread: () => void;
}

interface TreeNodeProps {
  thread: ThreadWithId;
  threads: ThreadWithId[];
  activeThreadId: string | null;
  onSelect: (threadId: string) => void;
  depth: number;
  expandedIds: Set<string>;
  onToggleExpand: (threadId: string) => void;
}

const TreeNode: React.FC<TreeNodeProps> = ({
  thread,
  threads,
  activeThreadId,
  onSelect,
  depth,
  expandedIds,
  onToggleExpand,
}) => {
  const children = threads.filter(t => {
    if (t.parent && typeof t.parent === 'object' && 'Thread' in t.parent) {
      return t.parent.Thread === thread.id;
    }
    return false;
  });

  const hasChildren = children.length > 0;
  const isExpanded = expandedIds.has(thread.id);
  const isActive = thread.id === activeThreadId;

  const handleExpandClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleExpand(thread.id);
  };

  const handleSelectClick = () => {
    onSelect(thread.id);
  };

  return (
    <div className="thread-tree-node">
      <div
        className={`thread-tree-row ${isActive ? 'active' : ''}`}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        {hasChildren ? (
          <button
            className="thread-expand-btn"
            onClick={handleExpandClick}
            aria-label={isExpanded ? 'Collapse' : 'Expand'}
          >
            {isExpanded ? '▼' : '▶'}
          </button>
        ) : (
          <span className="thread-expand-spacer" />
        )}
        <button className="thread-name-btn" onClick={handleSelectClick}>
          <span className="thread-name">
            {thread.title || (thread.depth === 0 ? 'Main' : `Thread ${thread.id.slice(-6)}`)}
          </span>
          <span className="thread-meta">
            {thread.summary?.message_count ?? 0} msgs
          </span>
        </button>
      </div>
      {hasChildren && isExpanded && (
        <div className="thread-tree-children">
          {children.map(child => (
            <TreeNode
              key={child.id}
              thread={child}
              threads={threads}
              activeThreadId={activeThreadId}
              onSelect={onSelect}
              depth={depth + 1}
              expandedIds={expandedIds}
              onToggleExpand={onToggleExpand}
            />
          ))}
        </div>
      )}
    </div>
  );
};

const ThreadsDropdown: React.FC<ThreadsDropdownProps> = ({
  threads,
  activeThreadId,
  rootThreadId,
  onSelect,
  onClose,
  canCreateThread,
  onCreateThread,
}) => {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => {
    // Start with root thread expanded
    const initial = new Set<string>();
    if (rootThreadId) initial.add(rootThreadId);
    return initial;
  });
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  const handleToggleExpand = (threadId: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(threadId)) {
        next.delete(threadId);
      } else {
        next.add(threadId);
      }
      return next;
    });
  };

  const handleSelect = (threadId: string) => {
    onSelect(threadId);
    onClose();
  };

  // Find root-level threads (depth 0 or parent is Root)
  const rootThreads = threads.filter(t => {
    if (t.depth === 0) return true;
    if (t.parent && typeof t.parent === 'object' && 'Root' in t.parent) return true;
    return false;
  });

  return (
    <div className="threads-dropdown" ref={dropdownRef}>
      <div className="threads-dropdown-header">
        <span className="threads-dropdown-title">Threads</span>
        {canCreateThread && (
          <button className="threads-dropdown-add" onClick={onCreateThread}>
            + New
          </button>
        )}
      </div>
      <div className="threads-dropdown-list">
        {rootThreads.length === 0 ? (
          <div className="threads-dropdown-empty">No threads yet</div>
        ) : (
          rootThreads.map(thread => (
            <TreeNode
              key={thread.id}
              thread={thread}
              threads={threads}
              activeThreadId={activeThreadId}
              onSelect={handleSelect}
              depth={0}
              expandedIds={expandedIds}
              onToggleExpand={handleToggleExpand}
            />
          ))
        )}
      </div>
    </div>
  );
};

export default ThreadsDropdown;
