import React, { useMemo, useState } from 'react';
import '../Chat/MessageMenu.css';

interface ChatItemMenuProps {
  itemKind: 'dm' | 'group' | 'spider';
  itemTitle: string;
  position: { x: number; y: number };
  onClose: () => void;
  onDelete?: () => void;
  onLeave?: () => void;
  onMarkUnread?: () => void;
  onTogglePin: () => void;
  isPinned: boolean;
}

const ChatItemMenu: React.FC<ChatItemMenuProps> = ({
  itemKind,
  itemTitle,
  position,
  onClose,
  onDelete,
  onLeave,
  onMarkUnread,
  onTogglePin,
  isPinned,
}) => {
  const [showConfirm, setShowConfirm] = useState(false);

  const menuStyle = useMemo(() => {
    const menuHeight = 180;
    const menuWidth = 180;
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

  const handleDeleteOrLeave = () => {
    setShowConfirm(true);
  };

  const handleConfirm = () => {
    if (itemKind === 'dm' && onDelete) {
      onDelete();
    } else if (itemKind === 'group' && onLeave) {
      onLeave();
    }
    onClose();
  };

  // Spider cannot be deleted/left or marked unread
  if (itemKind === 'spider') {
    return (
      <>
        <div className="menu-overlay" onClick={onClose} />
        <div className="message-menu" style={menuStyle}>
          <button onClick={() => { onTogglePin(); onClose(); }}>
            {isPinned ? 'Unpin' : 'Pin'}
          </button>
        </div>
      </>
    );
  }

  if (showConfirm) {
    return (
      <>
        <div className="menu-overlay" onClick={onClose} />
        <div className="message-menu chat-item-confirm" style={menuStyle}>
          <div className="confirm-header">
            {itemKind === 'dm' ? 'Delete chat?' : 'Leave group?'}
          </div>
          <div className="confirm-text">
            {itemKind === 'dm'
              ? `Delete your conversation with ${itemTitle}?`
              : `Leave "${itemTitle}"?`}
          </div>
          <div className="confirm-actions">
            <button className="cancel-btn" onClick={() => setShowConfirm(false)}>
              Cancel
            </button>
            <button className="danger-btn" onClick={handleConfirm}>
              {itemKind === 'dm' ? 'Delete' : 'Leave'}
            </button>
          </div>
        </div>
      </>
    );
  }

  return (
    <>
      <div className="menu-overlay" onClick={onClose} />
      <div className="message-menu" style={menuStyle}>
        <button onClick={() => { onTogglePin(); onClose(); }}>
          {isPinned ? 'Unpin' : 'Pin'}
        </button>
        {onMarkUnread && (
          <button onClick={() => { onMarkUnread(); onClose(); }}>
            Mark as unread
          </button>
        )}
        {(itemKind === 'dm' && onDelete) || (itemKind === 'group' && onLeave) ? (
          <button className="danger" onClick={handleDeleteOrLeave}>
            {itemKind === 'dm' ? 'Delete chat' : 'Leave group'}
          </button>
        ) : null}
      </div>
    </>
  );
};

export default ChatItemMenu;
