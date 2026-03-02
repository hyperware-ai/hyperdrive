import React from 'react';
import './NewChatButton.css';

interface NewChatButtonProps {
  onClick: () => void;
}

const NewChatButton: React.FC<NewChatButtonProps> = ({ onClick }) => {
  return (
    <button className="new-chat-button" onClick={onClick} aria-label="New Chat">
      <span className="plus-icon">+</span>
    </button>
  );
};

export default NewChatButton;