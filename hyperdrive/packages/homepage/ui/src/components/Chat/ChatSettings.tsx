import React from 'react';
import { Chat } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import './ChatSettings.css';

interface ChatSettingsProps {
  chat: Chat.Chat;
  onClose: () => void;
}

const ChatSettings: React.FC<ChatSettingsProps> = ({ chat, onClose }) => {
  const { deleteChat, updateChatSettings } = useChatStore();

  const handleBlockToggle = () => {
    // TODO: Implement block functionality
    console.log('Block toggle');
  };

  const handleNotifyToggle = async () => {
    await updateChatSettings(chat.id, { notify: !chat.notify });
  };

  const handleDeleteChat = async () => {
    if (confirm('Are you sure you want to delete this chat?')) {
      await deleteChat(chat.id);
      onClose();
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="chat-settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h3>Chat Settings</h3>
          <button className="close-button" onClick={onClose}>×</button>
        </div>
        
        <div className="settings-content">
          <div className="setting-item">
            <label>
              <input
                type="checkbox"
                checked={chat.notify}
                onChange={handleNotifyToggle}
              />
              <span>Notifications</span>
            </label>
          </div>
          
          <div className="setting-item">
            <label>
              <input
                type="checkbox"
                checked={chat.is_blocked}
                onChange={handleBlockToggle}
              />
              <span>Block User</span>
            </label>
          </div>
          
          <button className="delete-chat-button" onClick={handleDeleteChat}>
            Delete Chat
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChatSettings;