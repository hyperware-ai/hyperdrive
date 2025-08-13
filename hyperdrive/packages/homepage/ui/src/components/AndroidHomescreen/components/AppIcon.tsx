import React, { useState } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useNavigationStore } from '../../../stores/navigationStore';
import classNames from 'classnames';

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
  const [isPressed, setIsPressed] = useState(false);

  const handlePress = () => {
    if (!isEditMode && app.path) {
      openApp(app);
    }
  };

  return (
    <div
      className={classNames('app-icon relative flex gap-1 flex-col items-center justify-center  rounded-2xl cursor-pointer select-none transition-all', {
        'scale-95': isPressed,
        'scale-100': !isPressed,
        'animate-wiggle': isEditMode && isFloating,
        'hover:scale-110': !isEditMode && isFloating,
        'opacity-50': !app.path && !(app.process && app.publisher) && !app.base64_icon,
        'p-2': showLabel,
      })}
      onMouseDown={() => setIsPressed(true)}
      onMouseUp={() => setIsPressed(false)}
      onMouseLeave={() => setIsPressed(false)}
      onClick={handlePress}
      data-app-id={app.id}
      data-app-path={app.path}
      data-app-process={app.process}
      data-app-publisher={app.publisher}
    >

      <div className={classNames("rounded-xl w-16 h-16 overflow-hidden flex items-center justify-center shadow-lg", {
        'mb-1': showLabel,
      })}>
        {app.base64_icon ? (
          <img src={app.base64_icon} alt={app.label} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full flex items-center justify-center bg-iris">
            <span className="text-2xl text-white font-bold">{app.label?.[0]?.toUpperCase() || ''}{app.label?.[1]?.toLowerCase() || ''}</span>
          </div>
        )}
      </div>

      {showLabel && (
        <span className="text-xs text-center max-w-full truncate px-2 py-1 bg-black/5 dark:bg-white/5 rounded-full backdrop-blur-xl">
          {app.label}
        </span>
      )}
    </div>
  );
};