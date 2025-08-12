import React, { useEffect, useState } from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';
import classNames from 'classnames';
import { BsChevronLeft, BsClock, BsHouse } from 'react-icons/bs';

export const GestureZone: React.FC = () => {
  const { toggleRecentApps, runningApps, currentAppId, switchToApp, isRecentAppsOpen, closeAllOverlays } = useNavigationStore();
  const [touchStart, setTouchStart] = useState<{ x: number; y: number } | null>(null);
  const [isActive, setIsActive] = useState(false);
  const [_isHovered, setIsHovered] = useState(false);

  // Touch handlers
  const handleTouchStart = (e: React.TouchEvent) => {
    const touch = e.touches[0];
    setTouchStart({ x: touch.clientX, y: touch.clientY });
    setIsActive(true);
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (!touchStart) return;

    const touch = e.touches[0];
    const deltaX = touchStart.x - touch.clientX;
    const deltaY = touch.clientY - touchStart.y;

    // Swipe left (show recent apps)
    if (deltaX > 50 && Math.abs(deltaY) < 30) {
      toggleRecentApps();
      setTouchStart(null);
    }

    // Swipe up/down (switch apps)
    if (Math.abs(deltaY) > 50 && Math.abs(deltaX) < 30) {
      const currentIndex = runningApps.findIndex(app => app.id === currentAppId);
      if (currentIndex !== -1) {
        const newIndex = deltaY > 0
          ? Math.min(currentIndex + 1, runningApps.length - 1)
          : Math.max(currentIndex - 1, 0);
        if (newIndex !== currentIndex) {
          switchToApp(runningApps[newIndex].id);
        }
      }
      setTouchStart(null);
    }
  };

  const handleTouchEnd = () => {
    setTouchStart(null);
    setIsActive(false);
  };

  // Desktop click handler
  const handleClick = () => {
    toggleRecentApps();
  };

  useEffect(() => {
    if (!isRecentAppsOpen) {
      setTouchStart(null);
      setIsActive(false);
    }
  }, [isRecentAppsOpen]);

  return (
    <>
      <div
        className={classNames("gesture-zone fixed right-0 w-12 z-40 transition-transform cursor-pointer",
          {
            'bg-radial-[at_100%_50%] from-black/10 dark:from-white/10 to-transparent w-12 h-full top-0': isActive,
            'flex flex-col place-items-center place-content-center  h-1/2 top-1/4': !isActive
          })}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {!isActive && <div className="bg-black/20 dark:bg-white/20  backdrop-blur-xl px-1 py-3 rounded-l-xl flex flex-col self-end">
          <button
            onClick={handleClick}
            className="thin !bg-black/10 dark:!bg-white/10 !px-1 !py-2 dark:!text-white !rounded-b-none mb-px">
            <BsClock className="text-lg" />
          </button>
          <button
            onClick={closeAllOverlays}
            className="thin !bg-black/10 dark:!bg-white/10 !px-1 !py-2 dark:!text-white !rounded-t-none">
            <BsHouse className="text-lg" />
          </button>
        </div>}
      </div>

      {/* {isHovered && !isActive && (
        <div className="hidden md:block fixed right-12 top-1/2 transform -translate-y-1/2 bg-black/90 backdrop-blur text-white px-4 py-3 rounded-lg text-sm pointer-events-none z-50 shadow-xl">
          <div className="flex items-center gap-2 mb-1">
            <kbd className="px-2 py-1 bg-white/20 rounded text-xs">Click</kbd>
            <span>or</span>
            <kbd className="px-2 py-1 bg-white/20 rounded text-xs">S</kbd>
            <span>Recent apps</span>
          </div>
          <div className="flex items-center gap-2 mb-1">
            <kbd className="px-2 py-1 bg-white/20 rounded text-xs">A</kbd>
            <span>All apps</span>
          </div>
          <div className="flex items-center gap-2">
            <kbd className="px-2 py-1 bg-white/20 rounded text-xs">H</kbd>
            <span>Home</span>
          </div>
        </div>
      )} */}
    </>
  );
};