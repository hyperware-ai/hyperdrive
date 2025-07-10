import React, { useState } from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';

export const GestureZone: React.FC = () => {
  const { toggleRecentApps, runningApps, currentAppId, switchToApp } = useNavigationStore();
  const [touchStart, setTouchStart] = useState<{ x: number; y: number } | null>(null);
  const [isActive, setIsActive] = useState(false);
  const [isHovered, setIsHovered] = useState(false);

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

  return (
    <>
      <div
        className={`gesture-zone fixed right-0 top-0 w-8 h-full z-40 transition-all cursor-pointer
          ${isActive ? 'bg-white/20 w-12' : ''}
          ${isHovered && !isActive ? 'bg-gradient-to-l from-white/10 to-transparent' : ''}`}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onClick={handleClick}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      />
      {/* Desktop hint */}
      {isHovered && !isActive && (
        <div className="fixed right-12 top-1/2 transform -translate-y-1/2 bg-black/90 backdrop-blur text-white px-4 py-3 rounded-lg text-sm pointer-events-none z-50 shadow-xl">
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
      )}
    </>
  );
};