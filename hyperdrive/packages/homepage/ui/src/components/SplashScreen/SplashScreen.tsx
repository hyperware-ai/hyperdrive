import React, { useState } from 'react';
import ProfileButton from './ProfileButton';
import SettingsModal from '../Settings/SettingsModal';
import UnifiedMessages from './UnifiedMessages';
import './SplashScreen.css';

interface SplashScreenProps {
  showSpiderChat: boolean;
  setShowSpiderChat: (show: boolean) => void;
}

const SplashScreen: React.FC<SplashScreenProps> = ({
  showSpiderChat,
  setShowSpiderChat,
}) => {
  const [showSettings, setShowSettings] = useState(false);

  return (
    <div className="splash-screen">
      <div className="splash-header">
        <ProfileButton onClick={() => setShowSettings(true)} />
        <h1 className="app-title">Chats</h1>
        <button className="header-action" aria-label="New chat">
          <span className="material-symbols-outlined">edit_square</span>
        </button>
      </div>

      <div className="splash-content">
        <UnifiedMessages
          showSpiderChat={showSpiderChat}
          setShowSpiderChat={setShowSpiderChat}
        />
      </div>

      {showSettings && (
        <SettingsModal onClose={() => setShowSettings(false)} />
      )}
    </div>
  );
};

export default SplashScreen;
