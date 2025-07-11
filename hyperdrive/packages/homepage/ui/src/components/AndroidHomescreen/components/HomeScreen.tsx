import React, { useMemo, useEffect } from 'react';
import { useAppStore } from '../../../stores/appStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { Draggable } from './Draggable';
import { AppIcon } from './AppIcon';
import { Widget } from './Widget';
import type { HomepageApp } from '../../../types/app.types';

export const HomeScreen: React.FC = () => {
  const { apps } = useAppStore();
  const { homeScreenApps, dockApps, appPositions, widgetSettings, toggleWidget, moveItem, backgroundImage, setBackgroundImage, addToDock, removeFromDock } = usePersistenceStore();
  const { isEditMode, setEditMode } = useAppStore();
  const { toggleAppDrawer } = useNavigationStore();
  const [draggedAppId, setDraggedAppId] = React.useState<string | null>(null);
  const [touchDragPosition, setTouchDragPosition] = React.useState<{ x: number; y: number } | null>(null);
  const [showEditPanels, setShowEditPanels] = React.useState(false);

  const handleImageUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const dataUrl = event.target?.result as string;
        setBackgroundImage(dataUrl);
      };
      reader.readAsDataURL(file);
    }
  };

  const handleDockDrop = (e: React.DragEvent, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    const appId = e.dataTransfer.getData('appId');
    if (appId) {
      // Add to dock at the specified index
      // The addToDock function handles removing from existing position if needed
      addToDock(appId, index);
    }
  };

  const handleDockDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  };

  // Touch drag handlers for mobile
  const handleTouchStart = (appId: string) => (e: React.TouchEvent) => {
    if (!isEditMode) return;
    e.stopPropagation();
    setDraggedAppId(appId);
    const touch = e.touches[0];
    setTouchDragPosition({ x: touch.clientX, y: touch.clientY });
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (!draggedAppId || !touchDragPosition) return;
    e.preventDefault();
    const touch = e.touches[0];
    setTouchDragPosition({ x: touch.clientX, y: touch.clientY });
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    if (!draggedAppId || !touchDragPosition) return;
    
    const touch = e.changedTouches[0];
    const element = document.elementFromPoint(touch.clientX, touch.clientY);
    
    // Check if dropped on dock area
    const dockElement = element?.closest('.dock-area');
    if (dockElement) {
      // Find which dock slot was targeted
      const dockSlots = dockElement.querySelectorAll('[data-dock-index]');
      let targetIndex = dockApps.length;
      
      dockSlots.forEach((slot, index) => {
        const rect = slot.getBoundingClientRect();
        if (touch.clientX >= rect.left && touch.clientX <= rect.right &&
            touch.clientY >= rect.top && touch.clientY <= rect.bottom) {
          targetIndex = index;
        }
      });
      
      addToDock(draggedAppId, targetIndex);
    } else {
      // If not dropped on dock, just move the app to the new position
      const dockHeight = 120;
      const maxY = window.innerHeight - 80 - dockHeight;
      moveItem(draggedAppId, { 
        x: touch.clientX - 40, 
        y: Math.min(touch.clientY - 40, maxY) 
      });
    }
    
    setDraggedAppId(null);
    setTouchDragPosition(null);
  };

  // Handle window resize to keep apps on screen
  useEffect(() => {
    const handleResize = () => {
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;

      // Check and reposition apps
      Object.entries(appPositions).forEach(([appId, position]) => {
        let needsUpdate = false;
        let newX = position.x;
        let newY = position.y;

        // Assuming app icons are roughly 80px wide/tall (including padding)
        const appSize = 80;
        const dockHeight = 120; // Reserve space for dock

        if (position.x + appSize > windowWidth) {
          newX = Math.max(0, windowWidth - appSize);
          needsUpdate = true;
        }

        if (position.y + appSize > windowHeight - dockHeight) {
          newY = Math.max(0, windowHeight - appSize - dockHeight);
          needsUpdate = true;
        }

        if (needsUpdate) {
          moveItem(appId, { x: newX, y: newY });
        }
      });

      // Check and reposition widgets
      Object.entries(widgetSettings).forEach(([appId, settings]) => {
        if (settings.position && settings.size) {
          let needsUpdate = false;
          let newX = settings.position.x;
          let newY = settings.position.y;

          if (settings.position.x + settings.size.width > windowWidth) {
            newX = Math.max(0, windowWidth - settings.size.width);
            needsUpdate = true;
          }

          if (settings.position.y + settings.size.height > windowHeight) {
            newY = Math.max(0, windowHeight - settings.size.height);
            needsUpdate = true;
          }

          if (needsUpdate) {
            usePersistenceStore.getState().setWidgetPosition(appId, { x: newX, y: newY });
          }
        }
      });
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [appPositions, widgetSettings, moveItem]);

  const homeApps = useMemo(() => {
    return apps.filter(app => homeScreenApps.includes(app.id));
  }, [apps, homeScreenApps]);

  const widgetApps = useMemo(() => {
    return homeApps.filter(app => app.widget && !widgetSettings[app.id]?.hide);
  }, [homeApps, widgetSettings]);

  // Get actual dock app objects from IDs
  const dockAppsList = useMemo(() => {
    return dockApps
      .map(id => apps.find(app => app.id === id))
      .filter(Boolean) as HomepageApp[];
  }, [apps, dockApps]);

  // Floating apps are all home apps that aren't in the dock
  const floatingApps = useMemo(() => {
    return homeApps.filter(app => !dockApps.includes(app.id));
  }, [homeApps, dockApps]);

  return (
    <div
      className="h-full w-full relative overflow-hidden"
      style={{
        backgroundColor: backgroundImage ? 'transparent' : '#353534',
        backgroundImage: backgroundImage ? `url(${backgroundImage})` : 'none',
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundRepeat: 'no-repeat',
        touchAction: 'none'
      }}
    >
      {/* Background overlay for better text readability */}
      {backgroundImage && (
        <div className="absolute inset-0 bg-black/20" />
      )}

      {/* Content */}
      <div
        className="relative z-10 h-full"
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'move';
        }}
        onDrop={(e) => {
          e.preventDefault();
          const appId = e.dataTransfer.getData('appId');
          // Only handle drops from dock apps or if dropping outside dock area
          const isDroppingOnDock = (e.target as HTMLElement).closest('.dock-area');
          if (appId && !isDroppingOnDock) {
            if (dockApps.includes(appId)) {
              removeFromDock(appId);
            }
            // Ensure dropped app doesn't go behind dock
            const dockHeight = 120;
            const maxY = window.innerHeight - 80 - dockHeight; // 80 is app icon height
            moveItem(appId, { 
              x: e.clientX - 40, 
              y: Math.min(e.clientY - 40, maxY) 
            });
          }
        }}
        onTouchMove={(e) => {
          const touch = e.touches[0];
          const element = document.elementFromPoint(touch.clientX, touch.clientY);
          if (element?.closest('.dock-area')) {
            e.preventDefault();
          }
        }}
      >
        {/* Floating apps on canvas */}
        {floatingApps.map(app => {
          const position = appPositions[app.id] || {
            x: Math.min(Math.random() * (window.innerWidth - 100), window.innerWidth - 80),
            y: Math.min(window.innerHeight - 200 - Math.random() * 100, window.innerHeight - 80)
          };

          return (
            <Draggable
              key={app.id}
              id={app.id}
              position={position}
              onMove={(pos) => moveItem(app.id, pos)}
              isEditMode={isEditMode}
            >
              <div
                onTouchStart={handleTouchStart(app.id)}
                onTouchMove={handleTouchMove}
                onTouchEnd={handleTouchEnd}
              >
                <AppIcon app={app} isEditMode={isEditMode} isFloating={true} />
              </div>
            </Draggable>
          );
        })}

        {/* Widgets */}
        {widgetApps.map(app => (
          <Widget key={app.id} app={app} />
        ))}

        {/* Dock at bottom */}
        <div
          className="dock-area absolute bottom-4 left-1/2 transform -translate-x-1/2"
          onDragOver={handleDockDragOver}
          onDrop={(e) => handleDockDrop(e, dockAppsList.length)}
        >
          <div className="bg-black/60 backdrop-blur-xl rounded-3xl p-3 flex items-center gap-2 shadow-2xl border border-white/20">
            {/* Dock slots */}
            {Array.from({ length: 4 }).map((_, index) => {
              const app = dockAppsList[index];
              return (
                <div
                  key={`slot-${index}`}
                  data-dock-index={index}
                  className="w-16 h-16 relative"
                  onDragOver={handleDockDragOver}
                  onDrop={(e) => {
                    e.stopPropagation();
                    handleDockDrop(e, index);
                  }}
                >
                  {app ? (
                    isEditMode ? (
                      <div
                        draggable
                        onDragStart={(e) => {
                          e.dataTransfer.setData('appId', app.id);
                          e.dataTransfer.effectAllowed = 'move';
                        }}
                        onDragEnd={() => {
                          // If dropped outside, it's handled by floating area
                        }}
                        onTouchStart={handleTouchStart(app.id)}
                        onTouchMove={handleTouchMove}
                        onTouchEnd={(e) => {
                          if (!draggedAppId || !touchDragPosition) return;
                          
                          const touch = e.changedTouches[0];
                          const element = document.elementFromPoint(touch.clientX, touch.clientY);
                          
                          // If not dropped on dock, remove from dock
                          if (!element?.closest('.dock-area')) {
                            removeFromDock(app.id);
                            // Place at drop position
                            const dockHeight = 120;
                            const maxY = window.innerHeight - 80 - dockHeight;
                            moveItem(app.id, { 
                              x: touch.clientX - 40, 
                              y: Math.min(touch.clientY - 40, maxY) 
                            });
                          }
                          
                          setDraggedAppId(null);
                          setTouchDragPosition(null);
                        }}
                      >
                        <AppIcon app={app} isEditMode={false} showLabel={false} />
                      </div>
                    ) : (
                      <AppIcon app={app} isEditMode={false} showLabel={false} />
                    )
                  ) : (
                    <div className="w-full h-full border-2 border-dashed border-white/20 rounded-2xl transition-all hover:border-white/40 hover:bg-white/5" />
                  )}
                </div>
              );
            })}
            <div className="w-px h-12 bg-white/20 mx-1" />
            <button
              onClick={toggleAppDrawer}
              className="w-16 h-16 bg-gradient-to-br from-gray-700 to-gray-800 backdrop-blur rounded-2xl flex items-center justify-center text-white text-2xl hover:from-gray-600 hover:to-gray-700 transition-all shadow-lg"
            >
              ⊞
            </button>
          </div>
        </div>

        {/* Touch drag preview */}
        {draggedAppId && touchDragPosition && (
          <div
            className="fixed z-50 pointer-events-none opacity-75"
            style={{
              left: touchDragPosition.x - 40,
              top: touchDragPosition.y - 40,
            }}
          >
            <AppIcon 
              app={apps.find(a => a.id === draggedAppId)!} 
              isEditMode={false} 
              showLabel={false} 
            />
          </div>
        )}

        {/* Edit mode toggle and widget settings */}
        <div className="absolute top-4 right-4 flex flex-col items-end gap-2">
          {!isEditMode && (
            <button
              onClick={() => setEditMode(true)}
              className="px-4 py-2 bg-white/10 backdrop-blur-xl rounded-full text-white text-sm font-medium hover:bg-white/20 transition-all shadow-lg border border-white/20"
            >
              Edit
            </button>
          )}

          {isEditMode && (
            <div className="flex flex-col items-end gap-2">
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowEditPanels(!showEditPanels)}
                  className="w-10 h-10 bg-white/10 backdrop-blur-xl rounded-full text-white hover:bg-white/20 transition-all shadow-lg border border-white/20 flex items-center justify-center"
                  title="Settings"
                >
                  ⚙️
                </button>
                <button
                  onClick={() => {
                    setEditMode(false);
                    setShowEditPanels(false);
                  }}
                  className="px-4 py-2 bg-gradient-to-r from-gray-600 to-gray-700 rounded-full text-white text-sm font-medium hover:shadow-lg transition-all shadow-lg"
                >
                  Done
                </button>
              </div>
              
              {showEditPanels && (
                <div className="flex items-start gap-2">
                  {/* Background Settings */}
                  <div className="bg-black/80 backdrop-blur-xl rounded-2xl p-4 shadow-2xl border border-white/20">
                <h3 className="text-white text-sm font-semibold mb-3">Background</h3>
                <div className="space-y-3">
                  <div>
                    <label className="text-white/80 text-xs">Upload Image:</label>
                    <input
                      type="file"
                      accept="image/*"
                      onChange={handleImageUpload}
                      className="hidden"
                      id="background-upload"
                    />
                    <label
                      htmlFor="background-upload"
                      className="mt-1 w-full px-3 py-2 bg-white/10 border border-white/20 rounded-lg text-white text-sm cursor-pointer hover:bg-white/20 transition-all flex items-center justify-center"
                    >
                      Choose Image
                    </label>
                  </div>
                  <div className="text-white/60 text-xs text-center">OR</div>
                  <div>
                    <label className="text-white/80 text-xs">Image URL:</label>
                    <input
                      type="text"
                      value={backgroundImage && !backgroundImage.startsWith('data:') ? backgroundImage : ''}
                      onChange={(e) => setBackgroundImage(e.target.value || null)}
                      placeholder="Enter image URL"
                      className="w-full mt-1 px-3 py-2 bg-white/10 border border-white/20 rounded-lg text-white text-sm placeholder-white/40 focus:outline-none focus:border-white/40"
                    />
                  </div>
                  {backgroundImage && (
                    <button
                      onClick={() => setBackgroundImage(null)}
                      className="w-full px-3 py-1.5 bg-red-500/30 hover:bg-red-500/50 rounded-lg text-white text-sm font-medium transition-all"
                    >
                      Remove Background
                    </button>
                  )}
                </div>
              </div>
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
                            : 'bg-gray-600/50 hover:bg-gray-600/70'
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
                </div>
              )}
            </div>
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
