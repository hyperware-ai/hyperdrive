import React, { useState } from 'react';
import { Chat } from '#caller-utils';
import { useChatStore } from '../../store/chat';
import Avatar from '../Common/Avatar';
import ChatSettings from './ChatSettings';
import './ChatHeader.css';

interface ChatHeaderProps {
  chat: Chat.Chat;
}

const ChatHeader: React.FC<ChatHeaderProps> = ({ chat }) => {
  const { setActiveChat } = useChatStore();
  const [showSettings, setShowSettings] = useState(false);
  const isOfficial = chat.counterparty === 'dao.hypr';

  return (
    <>
      <div className="chat-header">
        <button className="back-button" onClick={() => setActiveChat(null)}>
          ←
        </button>

        <div className={`chat-header-info ${isOfficial ? 'official-chat' : ''}`}>
          <div className="chat-avatar-wrap">
            <Avatar
              name={chat.counterparty}
              profilePic={chat.counterparty_profile?.profile_pic}
              size="small"
            />
            {isOfficial && <span className="official-glow" aria-hidden="true" />}
          </div>
          <span className="chat-header-name">
            {chat.counterparty}
            {isOfficial && <span className="official-badge">official</span>}
          </span>
        </div>

        <button className="settings-button" onClick={() => setShowSettings(true)} aria-label="Chat settings">
          ⚙️
        </button>
      </div>
      
      {showSettings && (
        <ChatSettings chat={chat} onClose={() => setShowSettings(false)} />
      )}
    </>
  );
};

export default ChatHeader;
