import React from 'react';
import './Avatar.css';

interface AvatarProps {
  name: string;
  profilePic?: string | null;
  size?: 'small' | 'medium' | 'large';
}

const Avatar: React.FC<AvatarProps> = ({ name, profilePic, size = 'medium' }) => {
  const getInitial = () => {
    return name?.charAt(0).toUpperCase() || '?';
  };

  return (
    <div className={`avatar avatar-${size}`}>
      {profilePic ? (
        <img src={profilePic} alt={name} className="avatar-image" />
      ) : (
        <div className="avatar-initial">{getInitial()}</div>
      )}
    </div>
  );
};

export default Avatar;