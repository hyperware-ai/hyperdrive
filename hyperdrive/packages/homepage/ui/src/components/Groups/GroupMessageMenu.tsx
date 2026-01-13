import React, { useMemo, useState } from 'react';
import { GroupMessage } from '../../types/groups';
import '../Chat/MessageMenu.css';
import GroupDeleteMessageModal from './GroupDeleteMessageModal';

interface ChildThreadInfo {
  id: string;
  title: string | null;
}

interface GroupMessageMenuProps {
  message: GroupMessage;
  position: { x: number; y: number };
  onClose: () => void;
  onStartThread?: (parentThreadId: string, rootMessageId?: string) => void | Promise<void>;
  onOpenThread?: (threadId: string) => void;
  onReply?: (messageId: string) => void;
  onSetEditingMessage?: (message: { id: string; content: string }) => void;
  onDelete?: (messageId: string) => void;
  onReact?: (messageId: string, emoji: string) => void;
  isActiveThread?: boolean;
  currentNode?: string | null;
  childThread?: ChildThreadInfo | null;
}

const GroupMessageMenu: React.FC<GroupMessageMenuProps> = ({
  message,
  position,
  onClose,
  onStartThread,
  onOpenThread,
  onReply,
  onSetEditingMessage,
  onDelete,
  onReact,
  currentNode,
  childThread,
}) => {
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const menuStyle = useMemo(() => {
    const menuHeight = 240;
    const menuWidth = 170;
    const padding = 10;

    let top = position.y;
    let left = position.x;

    if (top + menuHeight > window.innerHeight - padding) {
      top = Math.max(padding, position.y - menuHeight);
    }
    if (left + menuWidth > window.innerWidth - padding) {
      left = window.innerWidth - menuWidth - padding;
    }

    top = Math.max(padding, top);
    left = Math.max(padding, left);

    return { top, left };
  }, [position]);

  const handleCopy = () => {
    if (navigator.clipboard) {
      navigator.clipboard.writeText(message.content).catch(() => {});
    }
    onClose();
  };

  const handleReply = () => {
    if (onReply) {
      onReply(message.id);
      onClose();
    }
  };

  const handleEdit = () => {
    if (onSetEditingMessage) {
      onSetEditingMessage({ id: message.id, content: message.content });
      onClose();
    }
  };

  const handleDelete = () => {
    if (onDelete) {
      setShowDeleteModal(true);
    }
  };

  const handleConfirmDelete = () => {
    if (onDelete) {
      onDelete(message.id);
      onClose();
    }
  };

  const handleReact = () => {
    if (!onReact) return;
    setShowEmojiPicker(true);
  };

  const isMyMessage = currentNode && message.sender === currentNode;
  const commonEmojis = ['👍', '❤️', '😂', '😮', '😢', '😡', '👎', '⚡', '🔥', '💯'];

  return (
    <>
      <div className="menu-overlay" onClick={onClose} />
      <GroupDeleteMessageModal
        isOpen={showDeleteModal}
        onClose={() => {
          setShowDeleteModal(false);
          onClose();
        }}
        onDeleteForEveryone={handleConfirmDelete}
      />
      {showEmojiPicker ? (
        <div
          className="emoji-tray"
          style={{
            position: 'fixed',
            top: Math.min(position.y, window.innerHeight - 100),
            left: Math.min(position.x, window.innerWidth - 400),
            transform: 'translateY(-50%)',
          }}
        >
          {commonEmojis.map((emoji) => (
            <button
              key={emoji}
              className="emoji-tray-option"
              onClick={() => {
                if (onReact) {
                  onReact(message.id, emoji);
                }
                onClose();
              }}
            >
              {emoji}
            </button>
          ))}
          <button
            className="emoji-tray-more"
            onClick={() => {
              setShowEmojiPicker(false);
            }}
          >
            Cancel
          </button>
        </div>
      ) : (
        <div className="message-menu" style={menuStyle}>
          <button onClick={handleReply} disabled={!onReply}>
            Reply
          </button>
          {!childThread && onStartThread ? (
            <button
              onClick={() => {
                onStartThread(message.threadId, message.id);
                onClose();
              }}
            >
              Start thread
            </button>
          ) : null}
          <button onClick={handleCopy}>Copy</button>
          <button onClick={handleReact} disabled={!onReact}>
            React
          </button>
          {isMyMessage && onSetEditingMessage && !childThread && (
            <button onClick={handleEdit}>Edit</button>
          )}
          {isMyMessage && onDelete && !childThread && (
            <button onClick={handleDelete}>Delete</button>
          )}
        </div>
      )}
    </>
  );
};

export default GroupMessageMenu;
