import React from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';
import { BsX } from 'react-icons/bs';
import dayjs from 'dayjs';

export const RecentApps: React.FC = () => {
  const { runningApps, isRecentAppsOpen, switchToApp, closeApp, toggleRecentApps, closeAllOverlays } = useNavigationStore();

  if (!isRecentAppsOpen) return null;

  return (
    <div className="recent-apps fixed inset-0 bg-gradient-to-b from-gray-900/50 to-white/50 dark:to-black/50 backdrop-blur-xl z-50 flex items-center justify-center">
      {runningApps.length === 0 ? (
        <div className="text-center flex flex-col items-center justify-center gap-4">
          <div className="text-6xl">📱</div>
          <h2 className="text-xl opacity-70">No running apps</h2>
          <p className="opacity-50">Open an app to see it here</p>
          <button
            onClick={closeAllOverlays}
            className="!rounded-full"
          >
            Home
          </button>
        </div>
      ) : (
        <>
          <div className="w-full max-w-6xl h-[70vh] overflow-x-auto">
            <div className="flex gap-4 p-4 h-full items-center justify-center flex-wrap">
              {runningApps.map(app => (
                <div
                  key={app.id}
                  className="relative flex-shrink-0 w-72 h-96 bg-gradient-to-b from-black/10 to-black/20 dark:from-white/10 dark:to-white/20 rounded-3xl overflow-hidden cursor-pointer group transform transition-all hover:scale-105 hover:shadow-2xl"
                  onClick={() => switchToApp(app.id)}
                >
                  <div className="p-4 bg-gradient-to-r from-iris/20 dark:from-neon/20 to-transparent flex items-center justify-between">
                    <span className="font-medium">{app.label}</span>
                    <button
                      onClick={(e) => {
                        try {
                          e.stopPropagation();
                        } catch { }
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
                    {/* <div className="text-8xl mb-4 opacity-20">⧉</div>
                    <p className="text-lg">App Preview</p> */}

                    {app.base64_icon ? (
                      <img src={app.base64_icon} alt={app.label} className="aspect-square rounded-xl w-full object-cover" />
                    ) : (
                      <div className="aspect-square rounded-xl bg-gradient-to-br from-iris/40 dark:from-neon/40 to-transparent flex items-center justify-center text-white font-bold w-full ">
                        {app?.label?.[0]?.toUpperCase() + (app?.label?.length > 1 ? app.label?.[1]?.toLocaleLowerCase() : '')}
                      </div>
                    )}
                    <p className="text-sm mt-2 opacity-90">opened {dayjs(runningApps.find(a => a.id === app.id)?.openedAt || 0).fromNow()}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="absolute bottom-8 left-1/2 transform -translate-x-1/2 flex gap-4">
            <button
              onClick={closeAllOverlays}
              className="!px-6 !py-3 text-white font-medium hover:shadow-lg transition-all transform hover:scale-105 !rounded-full"
            >
              Home
            </button>
            <button
              onClick={toggleRecentApps}
              className="!px-6 !py-3 !bg-black/10 dark:!bg-white/10 backdrop-blur dark:!text-white hover:bg-white/20 transition-all !rounded-full"
            >
              Close
            </button>
          </div>
        </>
      )}
    </div>
  );
};