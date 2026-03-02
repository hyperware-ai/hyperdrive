import React from 'react';
import { Chat } from '#caller-utils';
import './ThreadsList.css';

type ThreadWithId = Chat.Thread & { id: string };

interface ThreadsListProps {
  threads: ThreadWithId[];
  activeThreadId: string | null;
  onSelect: (threadId: string) => void;
}

const ThreadsList: React.FC<ThreadsListProps> = ({ threads, activeThreadId, onSelect }) => {
  if (!threads.length) {
    return (
      <div className="threads-empty">
        No threads yet. Open a thread or start one from a message.
      </div>
    );
  }

  return (
    <div className="threads-list">
      {threads.map((thread) => (
        <button
          key={thread.id}
          className={`thread-row ${thread.id === activeThreadId ? 'active' : ''}`}
          onClick={() => onSelect(thread.id)}
        >
          <div className="thread-row-main">
            <div className="thread-row-title">
              {thread.title || (thread.depth === 0 ? 'Main thread' : thread.id)}
            </div>
            <div className="thread-row-meta">
              {thread.summary?.message_count ?? 0} msgs • Updated{' '}
              {thread.summary?.last_activity
                ? new Date(thread.summary.last_activity * 1000).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                  })
                : '—'}
            </div>
          </div>
          <div className="thread-row-depth">Depth {thread.depth}</div>
        </button>
      ))}
    </div>
  );
};

export default ThreadsList;
