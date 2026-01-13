import React, { useState } from 'react';
import ProfileButton from './ProfileButton';
import SettingsModal from '../Settings/SettingsModal';
import UnifiedMessages from './UnifiedMessages';
import './SplashScreen.css';

const SplashScreen: React.FC = () => {
  const [showSettings, setShowSettings] = useState(false);

  return (
    <div className="splash-screen">
      <div className="splash-header">
        <ProfileButton onClick={() => setShowSettings(true)} />
        <h1 className="app-title">Chat</h1>
        <div className="header-spacer" />
      </div>
      
      <div className="splash-content">
        <UnifiedMessages />
      </div>
      
      {showSettings && (
        <SettingsModal onClose={() => setShowSettings(false)} />
      )}
    </div>
  );
};

export default SplashScreen;
