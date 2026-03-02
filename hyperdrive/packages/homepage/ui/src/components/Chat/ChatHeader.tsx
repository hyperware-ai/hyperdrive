import React, { useState } from 'react';
import { Chat } from '#caller-utils';
import Avatar from '../Common/Avatar';
import ChatSettings from './ChatSettings';
import SendCryptoModal from './SendCryptoModal';
import {
  getChatDisplayName,
  getChatNodeSubtitle,
  getCounterpartyBaseAddress,
} from '../../utils/chatDisplay';
import './ChatHeader.css';

interface ChatHeaderProps {
  chat: Chat.Chat;
  onBack: () => void;
}

const ChatHeader: React.FC<ChatHeaderProps> = ({ chat, onBack }) => {
  const [showSettings, setShowSettings] = useState(false);
  const [showSendCrypto, setShowSendCrypto] = useState(false);
  const isOfficial = chat.counterparty === 'dao.hypr';
  const displayName = getChatDisplayName(chat);
  const nodeSubtitle = getChatNodeSubtitle(chat);
  const recipientAddress = getCounterpartyBaseAddress(chat);

  return (
    <>
      <div className="chat-header">
        <button className="back-button" onClick={onBack}>
          <span className="material-symbols-outlined" aria-hidden="true">
            arrow_back
          </span>
        </button>

        <div className={`chat-header-info ${isOfficial ? 'official-chat' : ''}`}>
          <div className="chat-avatar-wrap">
            <Avatar
              name={displayName}
              profilePic={chat.counterparty_profile?.profile_pic}
              size="small"
            />
            {isOfficial && <span className="official-glow" aria-hidden="true" />}
          </div>
          <div className="chat-header-title-wrap">
            <span className="chat-header-name">
              {displayName}
              {isOfficial && <span className="official-badge">official</span>}
            </span>
            <span className="chat-header-subtitle">{nodeSubtitle}</span>
          </div>
        </div>

        <div className="chat-header-actions">
          <button
            className="voice-call-button"
            onClick={() => setShowSendCrypto(true)}
            aria-label="Send crypto"
            disabled={!recipientAddress}
            title={recipientAddress ? 'Send crypto on Base' : 'No payment address set'}
          >
            <span className="material-symbols-outlined">payments</span>
          </button>
          <button className="settings-button" onClick={() => setShowSettings(true)} aria-label="Chat settings">
            <span className="material-symbols-outlined" aria-hidden="true">
              settings
            </span>
          </button>
        </div>
      </div>
      
      {showSettings && (
        <ChatSettings chat={chat} onClose={() => setShowSettings(false)} />
      )}
      {showSendCrypto && recipientAddress && (
        <SendCryptoModal
          chat={chat}
          recipientAddress={recipientAddress}
          onClose={() => setShowSendCrypto(false)}
        />
      )}
    </>
  );
};

export default ChatHeader;
