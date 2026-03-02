import React, { useState } from 'react';
import { useGroupStore } from '../../store/groups';
import './GroupJoinModal.css';

interface GroupJoinModalProps {
  host: string;
  keyValue: string;
  onClose: () => void;
}

const GroupJoinModal: React.FC<GroupJoinModalProps> = ({ host, keyValue, onClose }) => {
  const { joinGroupLink } = useGroupStore();
  const [isJoining, setIsJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleJoin = async () => {
    setIsJoining(true);
    setError(null);
    const groupId = await joinGroupLink(host, keyValue);
    if (groupId) {
      onClose();
    } else {
      setError('Unable to join this group. Try again.');
    }
    setIsJoining(false);
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content group-join-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Join public group</h3>
          <button className="close-button" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="modal-body group-join-body">
          <p className="group-join-text">
            This link was shared from <span className="group-join-host">{host}</span>.
          </p>
          {error && <div className="group-join-error">{error}</div>}
          <div className="group-join-actions">
            <button className="secondary" onClick={onClose} disabled={isJoining}>
              Cancel
            </button>
            <button className="primary" onClick={handleJoin} disabled={isJoining}>
              {isJoining ? 'Joining…' : 'Join group'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default GroupJoinModal;
