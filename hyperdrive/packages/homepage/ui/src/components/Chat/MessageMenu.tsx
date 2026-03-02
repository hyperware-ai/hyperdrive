import React, { useState } from 'react';
import { Chat } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import * as Caller from '#caller-utils';
import DeleteMessageModal from './DeleteMessageModal';
import './MessageMenu.css';

interface MessageMenuProps {
  message: Chat.ChatMessage;
  isOwn: boolean;
  position: { x: number; y: number };
  onClose: () => void;
}

const { add_reaction } = Caller.Chat;

const MessageMenu: React.FC<MessageMenuProps> = ({ message, isOwn, position, onClose }) => {
  const { deleteMessage, deleteMessageLocally, activeChat, setEditingMessage } = useChatStore();
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const isPaymentEvent = message.message_type === Chat.MessageType.Payment;
  const canEditOrDelete = isOwn && !isPaymentEvent;

  const commonEmojis = ['👍', '❤️', '😂', '😮', '😢', '😡', '👎', '⚡', '🔥', '💯'];

  const handleReply = () => {
    // Set the message as the one being replied to
    useChatStore.getState().setReplyingTo(message);
    onClose();
  };

  const handleCopy = () => {
    navigator.clipboard.writeText(message.content);
    onClose();
  };

  const handleEdit = () => {
    setEditingMessage({ id: message.id, content: message.content });
    onClose();
  };

  const handleDelete = () => {
    setShowDeleteModal(true);
  };
  
  const handleDeleteLocally = () => {
    deleteMessageLocally(message.id);
    onClose();
  };
  
  const handleDeleteForBoth = () => {
    deleteMessage(message.id);
    onClose();
  };
  
  const handleAddReaction = async (emoji: string) => {
    try {
      await add_reaction({
        chat_id: activeChat?.id || '',
        message_id: message.id,
        emoji
      });
      // WebSocket will handle the update
      onClose();
    } catch (err) {
      console.error('Error adding reaction:', err);
    }
  };

  // Calculate position to keep menu on screen
  const menuStyle = React.useMemo(() => {
    const menuHeight = canEditOrDelete ? 240 : 180; // Approximate menu height
    const menuWidth = 150; // Approximate menu width
    const padding = 10;
    
    let top = position.y;
    let left = position.x;
    
    // Check if menu would go off bottom of screen
    // If so, position it above the click point instead of below
    if (top + menuHeight > window.innerHeight - padding) {
      // Position menu above the click point
      top = Math.max(padding, position.y - menuHeight);
    }
    
    // Check if menu would go off right side of screen
    if (left + menuWidth > window.innerWidth - padding) {
      left = window.innerWidth - menuWidth - padding;
    }
    
    // Ensure menu doesn't go off top or left
    top = Math.max(padding, top);
    left = Math.max(padding, left);
    
    return { top, left };
  }, [position, canEditOrDelete]);

  return (
    <>
      <div className="menu-overlay" onClick={onClose} />
      
      <DeleteMessageModal
        isOpen={showDeleteModal}
        onClose={() => setShowDeleteModal(false)}
        onDeleteLocally={handleDeleteLocally}
        onDeleteForBoth={handleDeleteForBoth}
        isOwnMessage={isOwn}
      />
      
      {/* Emoji tray - shown when React is clicked, replaces menu */}
      {showEmojiPicker ? (
        <div 
          className="emoji-tray"
          style={{
            position: 'fixed',
            top: Math.min(position.y, window.innerHeight - 100), // Keep emoji tray on screen
            left: Math.min(position.x, window.innerWidth - 400), // Account for tray width
            transform: 'translateY(-50%)'
          }}
        >
          {commonEmojis.map(emoji => (
            <button 
              key={emoji} 
              className="emoji-tray-option"
              onClick={() => handleAddReaction(emoji)}
            >
              {emoji}
            </button>
          ))}
          <button 
            className="emoji-tray-more"
            onClick={() => {
              // TODO: Open full emoji picker
              alert('Full emoji picker coming soon!');
            }}
          >
            ➕
          </button>
        </div>
      ) : (
        <div 
          className="message-menu"
          style={menuStyle}
        >
          <button onClick={handleReply}>Reply</button>
          <button onClick={handleCopy}>Copy</button>
          <button onClick={() => setShowEmojiPicker(true)}>React</button>
          {canEditOrDelete && <button onClick={handleEdit}>Edit</button>}
          {canEditOrDelete && <button onClick={handleDelete}>Delete</button>}
        </div>
      )}
    </>
  );
};

export default MessageMenu;
