import React, { useState } from 'react';
import * as Caller from '#caller-utils';
import { useChatStore } from '../../store/chat';
import './CreateChatLink.css';

const { create_chat_link } = Caller.Chat;

interface CreateChatLinkProps {
  onClose: () => void;
}

export const CreateChatLink: React.FC<CreateChatLinkProps> = ({ onClose }) => {
  const { activeChat } = useChatStore();
  const [singleUse, setSingleUse] = useState(false);
  const [generatedLink, setGeneratedLink] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreateLink = async () => {
    setLoading(true);
    setError(null);
    try {
      if (!activeChat) {
        setError('No active chat selected');
        return;
      }
      const link = await create_chat_link({ 
        chat_id: activeChat.id,
        single_use: singleUse 
      });
      const fullLink = `${window.location.origin}/public/join-${link}`;
      setGeneratedLink(fullLink);
    } catch (err) {
      setError('Failed to create chat link');
      console.error('Error creating chat link:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleCopyLink = () => {
    if (generatedLink) {
      navigator.clipboard.writeText(generatedLink);
    }
  };

  return (
    <div className="create-chat-link-overlay" onClick={onClose}>
      <div className="create-chat-link-modal" onClick={e => e.stopPropagation()}>
        <h2>Create Chat Link</h2>
        
        {!generatedLink ? (
          <>
            <p>Generate a link that allows others to chat with you through their browser.</p>
            
            <label className="single-use-checkbox">
              <input
                type="checkbox"
                checked={singleUse}
                onChange={e => setSingleUse(e.target.checked)}
              />
              Single use link (can only be used once)
            </label>
            
            {error && <div className="error-message">{error}</div>}
            
            <div className="button-group">
              <button onClick={handleCreateLink} disabled={loading}>
                {loading ? 'Creating...' : 'Create Link'}
              </button>
              <button onClick={onClose} className="cancel-button">
                Cancel
              </button>
            </div>
          </>
        ) : (
          <>
            <p>Your chat link has been created!</p>
            
            <div className="link-display">
              <input 
                type="text" 
                value={generatedLink} 
                readOnly 
                onClick={e => (e.target as HTMLInputElement).select()}
              />
              <button onClick={handleCopyLink}>Copy</button>
            </div>
            
            <div className="button-group">
              <button onClick={() => setGeneratedLink(null)}>
                Create Another
              </button>
              <button onClick={onClose}>
                Done
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
};
