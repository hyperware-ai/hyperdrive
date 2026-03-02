import React, { useState } from 'react';
import { useChatStore } from '../../store/chat';
import Avatar from '../Common/Avatar';
import './ProfileSettings.css';

const ProfileSettings: React.FC = () => {
  const { profile, updateProfile } = useChatStore();
  const nodeId = (window as any).our?.node || '';
  const nodeName = nodeId.split('.')[0] || 'User';

  const [nickname, setNickname] = useState(profile?.name || nodeName);
  const [profilePic, setProfilePic] = useState(profile?.profile_pic || '');
  const [baseAddress, setBaseAddress] = useState(profile?.base_address || '');
  const [error, setError] = useState<string | null>(null);

  const isValidEvmAddress = (value: string) => /^0x[a-fA-F0-9]{40}$/.test(value);

  const handleSave = async () => {
    const trimmedAddress = baseAddress.trim();
    if (trimmedAddress && !isValidEvmAddress(trimmedAddress)) {
      setError('Base address must be a valid 0x Ethereum address.');
      return;
    }
    setError(null);

    await updateProfile({
      name: nickname.trim() || nodeName,
      profile_pic: profilePic || null,
      base_address: trimmedAddress || null,
    });
  };

  return (
    <div className="profile-settings">
      <div className="profile-preview">
        <Avatar name={nickname || nodeName} profilePic={profilePic || null} size="large" />
      </div>

      <div className="form-group">
        <label htmlFor="nickname">Nickname</label>
        <input
          type="text"
          id="nickname"
          value={nickname}
          onChange={(e) => setNickname(e.target.value)}
          placeholder="How others see your name"
        />
      </div>

      <div className="form-group">
        <label htmlFor="profilePic">Profile Picture URL</label>
        <input
          type="text"
          id="profilePic"
          value={profilePic}
          onChange={(e) => setProfilePic(e.target.value)}
          placeholder="Enter image URL (optional)"
        />
      </div>

      <div className="form-group">
        <label htmlFor="baseAddress">Base Address</label>
        <input
          type="text"
          id="baseAddress"
          value={baseAddress}
          onChange={(e) => setBaseAddress(e.target.value)}
          placeholder="0x..."
        />
      </div>

      {error && <div className="profile-settings-error">{error}</div>}

      <button className="save-button" onClick={handleSave}>
        Save Profile
      </button>
    </div>
  );
};

export default ProfileSettings;
