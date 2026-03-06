import React from 'react';
import '../Chat/DeleteMessageModal.css';

interface GroupDeleteMessageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onDeleteForEveryone: () => void;
}

const GroupDeleteMessageModal: React.FC<GroupDeleteMessageModalProps> = ({
  isOpen,
  onClose,
  onDeleteForEveryone,
}) => {
  if (!isOpen) return null;

  return (
    <div className="delete-modal-overlay" onClick={onClose}>
      <div className="delete-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Delete Message</h3>
        <p>This message will be deleted for everyone in the group.</p>

        <div className="delete-modal-buttons">
          <button
            className="delete-button delete-both"
            onClick={() => {
              onDeleteForEveryone();
              onClose();
            }}
          >
            Delete for everyone
          </button>

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

export default GroupDeleteMessageModal;
