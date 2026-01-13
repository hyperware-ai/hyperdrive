import React, { useState, useRef } from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';
import { BsX } from 'react-icons/bs';
import dayjs from 'dayjs';

export const RecentApps: React.FC = () => {
  const { runningApps, isRecentAppsOpen, switchToApp, closeApp, toggleRecentApps, closeAllOverlays } = useNavigationStore();

  // Touch handling state
  const [swipeStates, setSwipeStates] = useState<{ [key: string]: {
    translateX: number;
    opacity: number;
    isDragging: boolean;
  } }>({});
  const touchStartRef = useRef<{ [key: string]: { x: number; y: number } }>({});

  const handleTouchStart = (e: React.TouchEvent, appId: string) => {
    const touch = e.touches[0];
    touchStartRef.current[appId] = { x: touch.clientX, y: touch.clientY };

    setSwipeStates(prev => ({
      ...prev,
      [appId]: { ...prev[appId], isDragging: true }
    }));
  };

  const handleTouchMove = (e: React.TouchEvent, appId: string) => {
    if (!touchStartRef.current[appId]) return;

    const touch = e.touches[0];
    const startX = touchStartRef.current[appId].x;
    const deltaX = touch.clientX - startX;
    const deltaY = Math.abs(touch.clientY - touchStartRef.current[appId].y);

    // Only handle horizontal swipes (avoid interfering with vertical scrolling)
    if (deltaY > 30) return

    const maxTranslate = window.innerWidth * 0.4; // 40% of screen width
    const clampedDeltaX = Math.max(-maxTranslate, Math.min(maxTranslate, deltaX));
    const opacity = Math.max(0.3, 1 - Math.abs(clampedDeltaX) / maxTranslate);

    setSwipeStates(prev => ({
      ...prev,
      [appId]: {
        translateX: clampedDeltaX,
        opacity,
        isDragging: true
      }
    }));
  };

  const handleTouchEnd = (e: React.TouchEvent, appId: string) => {
    if (!touchStartRef.current[appId]) return;

    const currentSwipe = swipeStates[appId];
    const threshold = window.innerWidth * 0.25; // 25% of screen width to close

    if (currentSwipe && Math.abs(currentSwipe.translateX) > threshold) {
      // Close the app with animation
      const direction = currentSwipe.translateX > 0 ? 1 : -1;
      setSwipeStates(prev => ({
        ...prev,
        [appId]: {
          translateX: direction * window.innerWidth,
          opacity: 0,
          isDragging: false
        }
      }));

      // Close app after animation
      setTimeout(() => {
        closeApp(appId);
        setSwipeStates(prev => {
          const newState = { ...prev };
          delete newState[appId];
          return newState;
        });

        if (runningApps.length === 1) {
          closeAllOverlays();
        }
      }, 200);
    } else {
      // Snap back to original position
      setSwipeStates(prev => ({
        ...prev,
        [appId]: {
          translateX: 0,
          opacity: 1,
          isDragging: false
        }
      }));
    }

    delete touchStartRef.current[appId];
  };

  if (!isRecentAppsOpen) return null;

  return (
    <div
    onClick={closeAllOverlays}
    className="recent-apps fixed inset-0 bg-gradient-to-b from-gray-900/50 to-white/50 dark:to-black/50 backdrop-blur-xl z-50 flex items-center justify-center"
    >
      {runningApps.length === 0 ? (
        <div
        onClick={closeAllOverlays}
        className="text-center flex flex-col items-center justify-center gap-4"
        >
          <div className="text-6xl">📱</div>
          <h2 className="text-xl opacity-70">No running apps</h2>
          <p className="opacity-50">Open an app to see it here</p>
        </div>
      ) : (
        <>
          <div className="w-full max-w-6xl h-[70vh] overflow-x-auto">
            <div className="flex gap-4 p-4 h-full items-center justify-center flex-wrap">
              {runningApps.map(app => {
                const swipeState = swipeStates[app.id] || { translateX: 0, opacity: 1, isDragging: false };

                return (
                <div
                  key={app.id}
                  className={`
                    relative flex-shrink-0 w-72 h-96
                    bg-gradient-to-b from-black/10 to-black/20 dark:from-white/10 dark:to-white/20
                    rounded-3xl overflow-hidden cursor-pointer select-none
                     group hover:scale-105 hover:shadow-2xl
                     ${swipeState.isDragging ? '' : 'transition-all duration-200'}
                     `}
                  style={{
                    transform: `translateX(${swipeState.translateX}px) ${swipeState.isDragging ? '' : 'scale(1)'}`,
                    opacity: swipeState.opacity,
                    transition: swipeState.isDragging ? 'none' : 'all 0.2s ease-out',
                  }}
                  onClick={(e) => {
                    if (swipeState.isDragging) return;
                    try { e.stopPropagation(); } catch { }
                    try { e.preventDefault(); } catch { }
                    switchToApp(app.id);
                  }}
                  onTouchStart={(e) => handleTouchStart(e, app.id)}
                  onTouchMove={(e) => handleTouchMove(e, app.id)}
                  onTouchEnd={(e) => handleTouchEnd(e, app.id)}
                >
                  <div className="p-4 bg-gradient-to-r from-iris/20 dark:from-neon/20 to-transparent flex items-center justify-between">
                    <span className="font-medium">{app.label}</span>
                    <button
                      onClick={(e) => {
                        try { e.stopPropagation(); } catch { }
                        try { e.preventDefault(); } catch { }
                        closeApp(app.id);
                        if (runningApps.length === 1) {
                          closeAllOverlays();
                        }
                      }}
                      className="clear thin text-xl"
                    >
                      <BsX />
                    </button>
                  </div>

                  <div className="p-8 text-white/50 text-center flex flex-col items-center justify-center ">
                    {app.base64_icon ? (
                      <img src={app.base64_icon} alt={app.label} className="aspect-square rounded-xl w-full object-cover" />
                    ) : (
                      <div className="aspect-square rounded-xl bg-gradient-to-br from-iris/40 dark:from-neon/40 to-transparent flex items-center justify-center text-white font-bold w-full ">
                        {app?.label?.[0]?.toUpperCase() + (app?.label?.length > 1 ? app.label?.[1]?.toLocaleLowerCase() : '')}
                      </div>
                    )}
                    <p className="text-sm mt-2 opacity-90 text-black dark:text-white">opened {dayjs(runningApps.find(a => a.id === app.id)?.openedAt || 0).fromNow()}</p>
                  </div>
                </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </div>
  );
};