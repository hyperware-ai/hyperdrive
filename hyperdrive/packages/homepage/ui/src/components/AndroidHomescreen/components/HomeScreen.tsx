import React, { useMemo } from 'react';
import { useAppStore } from '../../../stores/appStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { Draggable } from './Draggable';
import { AppIcon } from './AppIcon';
import { Widget } from './Widget';

export const HomeScreen: React.FC = () => {
  const { apps } = useAppStore();
  const { homeScreenApps, appPositions, widgetSettings, toggleWidget, moveItem } = usePersistenceStore();
  const { isEditMode, setEditMode } = useAppStore();
  const { toggleAppDrawer } = useNavigationStore();

  const homeApps = useMemo(() => {
    return apps.filter(app => homeScreenApps.includes(app.id));
  }, [apps, homeScreenApps]);

  const widgetApps = useMemo(() => {
    return homeApps.filter(app => app.widget && !widgetSettings[app.id]?.hide);
  }, [homeApps, widgetSettings]);

  // Dock apps are the first 5 apps marked as favorites or just first 5
  const dockApps = useMemo(() => {
    const favoriteApps = homeApps.filter(app => app.favorite).slice(0, 5);
    return favoriteApps.length > 0 ? favoriteApps : homeApps.slice(0, 5);
  }, [homeApps]);

  // Floating apps are all home apps that aren't in the dock
  const floatingApps = useMemo(() => {
    const dockAppIds = dockApps.map(app => app.id);
    return homeApps.filter(app => !dockAppIds.includes(app.id));
  }, [homeApps, dockApps]);

  return (
    <div className="flex-1 relative bg-gradient-to-br from-purple-900 via-blue-900 to-black">
      {/* Animated background */}
      <div className="absolute inset-0">
        <div className="absolute inset-0 bg-black/40" />
        <div className="absolute top-0 left-0 w-96 h-96 bg-purple-500 rounded-full filter blur-3xl opacity-20 animate-pulse" />
        <div className="absolute bottom-0 right-0 w-96 h-96 bg-blue-500 rounded-full filter blur-3xl opacity-20 animate-pulse" />
      </div>

      {/* Content */}
      <div className="relative z-10 h-full">
        {/* Floating apps on canvas */}
        {floatingApps.map(app => {
          const position = appPositions[app.id] || {
            x: Math.random() * (window.innerWidth - 100),
            y: window.innerHeight - 200 - Math.random() * 100
          };

          return (
            <Draggable
              key={app.id}
              id={app.id}
              position={position}
              onMove={(pos) => moveItem(app.id, pos)}
              isEditMode={isEditMode}
            >
              <AppIcon app={app} isEditMode={isEditMode} isFloating={true} />
            </Draggable>
          );
        })}

        {/* Widgets */}
        {widgetApps.map(app => (
          <Widget key={app.id} app={app} />
        ))}

        {/* Dock at bottom */}
        <div className="absolute bottom-4 left-1/2 transform -translate-x-1/2">
          <div className="bg-black/60 backdrop-blur-xl rounded-3xl p-3 flex items-center gap-2 shadow-2xl border border-white/20">
            {dockApps.map(app => (
              <AppIcon key={app.id} app={app} isEditMode={false} showLabel={false} />
            ))}
            <div className="w-px h-12 bg-white/20 mx-1" />
            <button
              onClick={toggleAppDrawer}
              className="w-16 h-16 bg-gradient-to-br from-gray-700 to-gray-800 backdrop-blur rounded-2xl flex items-center justify-center text-white text-2xl hover:from-gray-600 hover:to-gray-700 transition-all shadow-lg"
            >
              ⊞
            </button>
          </div>
        </div>

        {/* Edit mode toggle and widget settings */}
        <div className="absolute top-4 right-4 flex items-start gap-2">
          {!isEditMode && (
            <button
              onClick={() => setEditMode(true)}
              className="px-4 py-2 bg-white/10 backdrop-blur-xl rounded-full text-white text-sm font-medium hover:bg-white/20 transition-all shadow-lg border border-white/20"
            >
              Edit
            </button>
          )}

          {isEditMode && (
            <>
              <div className="bg-black/80 backdrop-blur-xl rounded-2xl p-4 max-w-xs shadow-2xl border border-white/20">
                <h3 className="text-white text-sm font-semibold mb-3">Widget Manager</h3>
                <div className="space-y-2 max-h-64 overflow-y-auto">
                  {homeApps.filter(app => app.widget).map(app => (
                    <div key={app.id} className="flex items-center justify-between text-white/80 text-sm p-2 rounded-lg hover:bg-white/10 transition-colors">
                      <span>{app.label}</span>
                      <button
                        onClick={() => toggleWidget(app.id)}
                        className={`px-3 py-1 rounded-full text-xs font-medium transition-all ${
                          widgetSettings[app.id]?.hide
                            ? 'bg-white/10 hover:bg-white/20'
                            : 'bg-green-500/50 hover:bg-green-500/70'
                        }`}
                      >
                        {widgetSettings[app.id]?.hide ? 'Show' : 'Hide'}
                      </button>
                    </div>
                  ))}
                  {homeApps.filter(app => app.widget).length === 0 && (
                    <p className="text-white/50 text-sm text-center py-4">No apps with widgets on home screen</p>
                  )}
                </div>
              </div>

              <button
                onClick={() => setEditMode(false)}
                className="px-4 py-2 bg-gradient-to-r from-green-500 to-green-600 rounded-full text-white text-sm font-medium hover:shadow-lg transition-all shadow-lg"
              >
                Done
              </button>
            </>
          )}
        </div>

        {/* Desktop hint */}
        <div className="hidden md:block absolute bottom-32 left-4 text-white/30 text-xs bg-black/50 backdrop-blur rounded-lg px-3 py-2">
          <div className="flex items-center gap-4">
            <span><kbd className="px-2 py-1 bg-white/10 rounded text-xs">A</kbd> All apps</span>
            <span><kbd className="px-2 py-1 bg-white/10 rounded text-xs">S</kbd> Recent apps</span>
            <span><kbd className="px-2 py-1 bg-white/10 rounded text-xs">H</kbd> Home</span>
            <span><kbd className="px-2 py-1 bg-white/10 rounded text-xs">1-9</kbd> Switch apps</span>
          </div>
        </div>
      </div>
    </div>
  );
};