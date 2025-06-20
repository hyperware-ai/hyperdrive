import React from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';

export const RecentApps: React.FC = () => {
  const { runningApps, isRecentAppsOpen, switchToApp, closeApp, toggleRecentApps, closeAllOverlays } = useNavigationStore();

  if (!isRecentAppsOpen) return null;

  return (
    <div className="fixed inset-0 bg-gradient-to-b from-gray-900/98 to-black/98 backdrop-blur-xl z-50 flex items-center justify-center">
      {runningApps.length === 0 ? (
        <div className="text-center">
          <div className="text-6xl mb-4 text-white/30">📱</div>
          <h2 className="text-xl text-white/70 mb-2">No running apps</h2>
          <p className="text-white/50 mb-8">Open an app to see it here</p>
          <button
            onClick={closeAllOverlays}
            className="px-6 py-3 bg-gradient-to-r from-blue-500 to-purple-500 rounded-full text-white font-medium hover:shadow-lg transition-all transform hover:scale-105"
          >
            🏠 Back to Home
          </button>
        </div>
      ) : (
        <>
          <div className="w-full max-w-6xl h-[70vh] overflow-x-auto">
            <div className="flex gap-4 p-4 h-full items-center justify-center flex-wrap">
              {runningApps.map(app => (
                <div
                  key={app.id}
                  className="relative flex-shrink-0 w-72 h-96 bg-gradient-to-b from-gray-800 to-gray-900 rounded-3xl overflow-hidden cursor-pointer group transform transition-all hover:scale-105 hover:shadow-2xl"
                  onClick={() => switchToApp(app.id)}
                >
                  <div className="p-4 bg-gradient-to-r from-blue-500/20 to-purple-500/20 flex items-center justify-between">
                    <div className="flex items-center gap-3">
                      {app.base64_icon ? (
                        <img src={app.base64_icon} alt={app.label} className="w-10 h-10 rounded-xl" />
                      ) : (
                        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-400 to-blue-600 flex items-center justify-center text-white font-bold">
                          {app.label[0]}
                        </div>
                      )}
                      <span className="text-white font-medium">{app.label}</span>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        closeApp(app.id);
                      }}
                      className="text-white/50 hover:text-white transition-colors text-xl"
                    >
                      ×
                    </button>
                  </div>

                  <div className="p-8 text-white/50 text-center flex flex-col items-center justify-center h-full">
                    <div className="text-8xl mb-4 opacity-20">⧉</div>
                    <p className="text-lg">App Preview</p>
                    <p className="text-sm mt-2 opacity-50">Click to switch</p>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="absolute bottom-8 left-1/2 transform -translate-x-1/2 flex gap-4">
            <button
              onClick={closeAllOverlays}
              className="px-6 py-3 bg-gradient-to-r from-blue-500 to-purple-500 rounded-full text-white font-medium hover:shadow-lg transition-all transform hover:scale-105"
            >
              🏠 Home
            </button>
            <button
              onClick={toggleRecentApps}
              className="px-6 py-3 bg-white/10 backdrop-blur rounded-full text-white hover:bg-white/20 transition-all"
            >
              Close
            </button>
          </div>
        </>
      )}
    </div>
  );
};