import React, { useState } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { useNavigationStore } from '../../../stores/navigationStore';
import classNames from 'classnames';

interface AppIconProps {
  app: HomepageApp;
  isEditMode: boolean;
  isUndocked?: boolean;
  isFloating?: boolean;
}

export const AppIcon: React.FC<AppIconProps> = ({
  app,
  isEditMode,
  isUndocked = true,
  isFloating = false
}) => {
  const { openApp } = useNavigationStore();
  const [isPressed, setIsPressed] = useState(false);
  const [isHovered, setIsHovered] = useState(false);

  const handlePress = () => {
    if (!isEditMode && app.path && app.path !== null) {
      openApp(app);
    }
  };

  // Calculate scale based on state priority: pressed > hovered > default
  const getScale = () => {
    if (isPressed) return 'scale(0.94)';
    if (isHovered && !isEditMode && isFloating) return 'scale(1.08)';
    return 'scale(1)';
  };

  return (
    <div
      className={classNames('app-icon relative flex gap-1 flex-col items-center justify-center rounded-2xl cursor-pointer select-none', {
        'animate-wiggle': isEditMode && isFloating,
        'opacity-50': !app.path && !(app.process && app.publisher) && !app.base64_icon,
        'p-2': isUndocked,
      })}
      style={{
        transform: getScale(),
        transition: 'transform var(--duration-fast, 150ms) var(--ease-spring, cubic-bezier(0.34, 1.56, 0.64, 1))',
      }}
      onMouseDown={() => setIsPressed(true)}
      onMouseUp={() => setIsPressed(false)}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => { setIsPressed(false); setIsHovered(false); }}
      onTouchStart={() => setIsPressed(true)}
      onTouchEnd={() => setIsPressed(false)}
      onClick={handlePress}
      data-app-id={app.id}
      data-app-path={app.path}
      data-app-process={app.process}
      data-app-publisher={app.publisher}
    >

      <div className={classNames("rounded-xl w-14 h-14 md:w-16 md:h-16 overflow-hidden flex items-center justify-center shadow-lg relative", {
        'mb-1': isUndocked,
      })}>
        {app.base64_icon ? (
          <img src={app.base64_icon} alt={app.label} className="w-full h-full object-cover" />
        ) : (
          <div className="w-full h-full flex items-center justify-center bg-iris">
            <span className="text-2xl text-white font-bold">{app.label?.[0]?.toUpperCase() || ''}{app.label?.[1]?.toLowerCase() || ''}</span>
          </div>
        )}
      </div>

      <span
        className={classNames(" text-center max-w-full self-stretch truncate", {
          'text-xs px-2 py-1 bg-black/5 dark:bg-white/5 rounded-full backdrop-blur-xl': isUndocked,
          'text-[10px]': !isUndocked,
        })}>
        {app.label}
      </span>
    </div>
  );
};
