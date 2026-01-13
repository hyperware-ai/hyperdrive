import React, { useState } from 'react';
import { useChatStore } from '../../store/chat';
import Avatar from '../Common/Avatar';
import './ProfileSettings.css';

const ProfileSettings: React.FC = () => {
  const { profile, updateProfile } = useChatStore();
  const [profilePic, setProfilePic] = useState(profile?.profile_pic || '');
  
  // Use the node name for display purposes
  const nodeName = (window as any).our?.node?.split('.')[0] || 'User';

  const handleSave = async () => {
    await updateProfile({
      name: profile?.name || nodeName, // Keep existing name in backend
      profile_pic: profilePic || null,
    });
  };

  return (
    <div className="profile-settings">
      <div className="profile-preview">
        <Avatar name={nodeName} profilePic={profilePic || null} size="large" />
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
      
      <button className="save-button" onClick={handleSave}>
        Save Profile
      </button>
    </div>
  );
};

export default ProfileSettings;