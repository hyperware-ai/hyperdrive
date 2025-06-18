import React, { useState } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useNavigationStore } from '../../../stores/navigationStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';

interface AppIconProps {
  app: HomepageApp;
  isEditMode: boolean;
  showLabel?: boolean;
  isFloating?: boolean;
}

export const AppIcon: React.FC<AppIconProps> = ({ 
  app, 
  isEditMode, 
  showLabel = true, 
  isFloating = false 
}) => {
  const { openApp } = useNavigationStore();
  const { removeFromHomeScreen } = usePersistenceStore();
  const [isPressed, setIsPressed] = useState(false);

  const handlePress = () => {
    if (!isEditMode && (app.path || (app.process && app.publisher))) {
      openApp(app);
    }
  };

  const handleRemove = (e: React.MouseEvent) => {
    e.stopPropagation();
    removeFromHomeScreen(app.id);
  };

  return (
    <div
      className={`relative flex flex-col items-center justify-center p-2 rounded-xl cursor-pointer select-none transition-all
        ${isPressed ? 'scale-95' : 'scale-100'}
        ${isEditMode && isFloating ? 'animate-wiggle' : ''}
        ${!isEditMode && isFloating ? 'hover:scale-110' : ''}
        ${!app.path && !(app.process && app.publisher) ? 'opacity-50' : ''}`}
      onMouseDown={() => setIsPressed(true)}
      onMouseUp={() => setIsPressed(false)}
      onMouseLeave={() => setIsPressed(false)}
      onClick={handlePress}
    >
      {isEditMode && isFloating && (
        <button
          onClick={handleRemove}
          className="absolute -top-2 -right-2 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center text-xs z-10 shadow-lg hover:bg-red-600 transition-colors"
        >
          ×
        </button>
      )}

      <div className="w-16 h-16 mb-1 rounded-2xl overflow-hidden flex items-center justify-center shadow-lg">
        {app.base64_icon ? (
          <img src={app.base64_icon} alt={app.label} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full bg-gradient-to-br from-blue-500 to-blue-700 flex items-center justify-center">
            <span className="text-2xl text-white font-bold">{app.label[0]}</span>
          </div>
        )}
      </div>

      {showLabel && (
        <span className="text-xs text-center max-w-full truncate text-white drop-shadow-md mt-1">
          {app.label}
        </span>
      )}
    </div>
  );
};