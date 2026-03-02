import React from 'react';
import './DeleteMessageModal.css';

interface DeleteMessageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onDeleteLocally: () => void;
  onDeleteForBoth: () => void;
  isOwnMessage: boolean;
}

const DeleteMessageModal: React.FC<DeleteMessageModalProps> = ({
  isOpen,
  onClose,
  onDeleteLocally,
  onDeleteForBoth,
  isOwnMessage
}) => {
  if (!isOpen) return null;

  return (
    <div className="delete-modal-overlay" onClick={onClose}>
      <div className="delete-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Delete Message</h3>
        <p>Choose how you want to delete this message:</p>
        
        <div className="delete-modal-buttons">
          <button 
            className="delete-button delete-locally"
            onClick={() => {
              onDeleteLocally();
              onClose();
            }}
          >
            Delete for me
          </button>
          
          {isOwnMessage && (
            <button 
              className="delete-button delete-both"
              onClick={() => {
                onDeleteForBoth();
                onClose();
              }}
            >
              Delete for everyone
            </button>
          )}
          
          <button 
            className="delete-button cancel"
            onClick={onClose}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

export default DeleteMessageModal;