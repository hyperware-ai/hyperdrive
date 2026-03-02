import React from 'react';
import { SpiderMessage } from '../../types/spider';
import '../Chat/MessageMenu.css';

interface SpiderMessageMenuProps {
  message: SpiderMessage;
  position: { x: number; y: number };
  onClose: () => void;
  onReply: (message: SpiderMessage) => void;
}

const SpiderMessageMenu: React.FC<SpiderMessageMenuProps> = ({
  message,
  position,
  onClose,
  onReply,
}) => {
  const handleReply = () => {
    onReply(message);
    onClose();
  };

  const handleCopy = () => {
    const text = message.content?.text || '';
    if (text) {
      navigator.clipboard.writeText(text);
    }
    onClose();
  };

  const menuStyle = React.useMemo(() => {
    const menuHeight = 100;
    const menuWidth = 160;
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

  return (
    <>
      <div className="menu-overlay" onClick={onClose} />
      <div className="message-menu" style={menuStyle}>
        <button onClick={handleReply}>Reply</button>
        <button onClick={handleCopy}>Copy</button>
      </div>
    </>
  );
};

export default SpiderMessageMenu;
