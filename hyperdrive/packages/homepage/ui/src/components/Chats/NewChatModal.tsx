import React, { useState } from 'react';
import { useChatStore } from '../../store/chat';
import './NewChatModal.css';

interface NewChatModalProps {
  onClose: () => void;
}

const NewChatModal: React.FC<NewChatModalProps> = ({ onClose }) => {
  const [counterparty, setCounterparty] = useState('');
  const { createChat } = useChatStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (counterparty.trim()) {
      await createChat(counterparty.trim());
      onClose();
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>New Chat</h2>
          <button className="close-button" onClick={onClose}>×</button>
        </div>
        
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label htmlFor="counterparty">Node address:</label>
            <input
              type="text"
              id="counterparty"
              value={counterparty}
              onChange={(e) => setCounterparty(e.target.value)}
              placeholder="e.g., alice.os"
              autoFocus
            />
          </div>
          
          <div className="modal-actions">
            <button type="button" onClick={onClose} className="cancel-button">
              Cancel
            </button>
            <button type="submit" className="submit-button" disabled={!counterparty.trim()}>
              Start Chat
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default NewChatModal;