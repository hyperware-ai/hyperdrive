import React, { useState } from 'react';
import { Chat } from '#caller-utils';
import { useGroupStore } from '../../store/groups';
import './GroupCreateModal.css';

interface GroupCreateModalProps {
  onClose: () => void;
}

const GroupCreateModal: React.FC<GroupCreateModalProps> = ({ onClose }) => {
  const { createGroup, isLoading } = useGroupStore();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [visibility, setVisibility] = useState<Chat.GroupVisibility>(
    Chat.GroupVisibility.Private,
  );
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    setError(null);
    const groupId = await createGroup({
      name: name.trim(),
      description: description.trim() || undefined,
      visibility,
      rootThreadTitle: null,
    });
    if (groupId) {
      onClose();
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Create group</h3>
          <button className="close-button" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="modal-body group-create-form">
          <label>
            <span>Group name</span>
            <input
              type="text"
              placeholder="Team updates"
              data-testid="group-name-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label>
            <span>Description</span>
            <textarea
              placeholder="What is this group for? (e.g., announcements)"
              data-testid="group-description-input"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
            />
          </label>
          <label>
            <span>Visibility</span>
            <select
              value={visibility}
              onChange={(e) =>
                setVisibility(e.target.value as unknown as Chat.GroupVisibility)
              }
            >
              <option value={Chat.GroupVisibility.Private}>Private</option>
              <option value={Chat.GroupVisibility.Public}>Public</option>
            </select>
          </label>
          {error && <div className="group-create-error">{error}</div>}
          <div className="group-create-actions">
            <button className="secondary" onClick={onClose}>
              Cancel
            </button>
            <button
              className="primary"
              onClick={handleCreate}
              disabled={isLoading}
              data-testid="group-create-submit"
            >
              {isLoading ? 'Creating…' : 'Create'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default GroupCreateModal;
