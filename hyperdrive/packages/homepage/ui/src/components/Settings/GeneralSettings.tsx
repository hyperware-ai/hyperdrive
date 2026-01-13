import React from 'react';
import { useChatStore } from '../../store/chat';
import { CHAT_APP_VERSION } from '../../constants/version';
import './GeneralSettings.css';

const GeneralSettings: React.FC = () => {
  const { settings, updateSettings } = useChatStore();

  const handleToggle = (key: keyof typeof settings) => {
    updateSettings({
      ...settings,
      [key]: !settings[key],
    });
  };

  const handleSTTKeyChange = (value: string) => {
    updateSettings({
      ...settings,
      stt_api_key: value || null,
    });
  };

  const handleFileSizeChange = (value: string) => {
    const sizeInMB = parseInt(value) || 10;
    updateSettings({
      ...settings,
      max_file_size_mb: Math.max(1, Math.min(100, sizeInMB)), // Limit between 1-100 MB
    });
  };

  return (
    <div className="general-settings">
      <div className="setting-item">
        <label>
          <input
            type="checkbox"
            checked={settings.show_images}
            onChange={() => handleToggle('show_images')}
          />
          <span>Show images in chats</span>
        </label>
      </div>

      <div className="setting-item">
        <label>
          <input
            type="checkbox"
            checked={settings.show_profile_pics}
            onChange={() => handleToggle('show_profile_pics')}
          />
          <span>Show profile pictures</span>
        </label>
      </div>

      <div className="setting-item">
        <label>
          <input
            type="checkbox"
            checked={settings.allow_browser_chats}
            onChange={() => handleToggle('allow_browser_chats')}
          />
          <span>Allow browser chats</span>
        </label>
      </div>

      <div className="setting-item">
        <label>
          <input
            type="checkbox"
            checked={settings.stt_enabled}
            onChange={() => handleToggle('stt_enabled')}
          />
          <span>Enable Speech-to-Text for voice notes</span>
        </label>
      </div>

      {settings.stt_enabled && (
        <div className="setting-item">
          <label>STT API Key:</label>
          <input
            type="text"
            value={settings.stt_api_key || ''}
            onChange={(e) => handleSTTKeyChange(e.target.value)}
            placeholder="Enter your STT API key"
          />
        </div>
      )}

      <div className="setting-item">
        <label>Max file size (MB):</label>
        <input
          type="number"
          min="1"
          max="100"
          value={settings.max_file_size_mb || 10}
          onChange={(e) => handleFileSizeChange(e.target.value)}
          style={{ width: '80px' }}
        />
        <span style={{ marginLeft: '10px', fontSize: '0.9em', color: '#666' }}>
          (1-100 MB)
        </span>
      </div>

      <div className="setting-item">
        <label>Version: {CHAT_APP_VERSION}</label>
      </div>
    </div>
  );
};

export default GeneralSettings;
