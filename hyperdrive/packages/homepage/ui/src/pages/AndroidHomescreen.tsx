import React, { useState, useEffect, useRef, useMemo } from 'react';
import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

// ==================== TYPES ====================
interface HomepageApp {
  id: string;
  process: string;
  package_name: string;
  publisher: string;
  path?: string;
  label: string;
  base64_icon?: string;
  widget?: string;
  order: number;
  favorite: boolean;
}

interface RunningApp extends HomepageApp {
  openedAt: number;
}

interface Position {
  x: number;
  y: number;
}

interface Size {
  width: number;
  height: number;
}

// ==================== STORES ====================
interface HomepageStore {
  apps: HomepageApp[];
  setApps: (apps: HomepageApp[]) => void;
  isEditMode: boolean;
  setEditMode: (mode: boolean) => void;
}

const useHomepageStore = create<HomepageStore>((set) => ({
  apps: [],
  setApps: (apps) => set({ apps }),
  isEditMode: false,
  setEditMode: (isEditMode) => set({ isEditMode }),
}));

interface NavigationStore {
  runningApps: RunningApp[];
  currentAppId: string | null;
  isAppDrawerOpen: boolean;
  isRecentAppsOpen: boolean;

  openApp: (app: HomepageApp) => void;
  closeApp: (appId: string) => void;
  switchToApp: (appId: string) => void;
  toggleAppDrawer: () => void;
  toggleRecentApps: () => void;
  closeAllOverlays: () => void;
}

const useNavigationStore = create<NavigationStore>((set, get) => ({
  runningApps: [],
  currentAppId: null,
  isAppDrawerOpen: false,
  isRecentAppsOpen: false,

  openApp: (app) => {
    // Don't open apps without a valid path
    if (!app.path && !app.process && !app.publisher) {
      console.warn(`Cannot open app ${app.label}: No valid path`);
      return;
    }

    const { runningApps } = get();
    const existingApp = runningApps.find(a => a.id === app.id);

    if (existingApp) {
      set({ currentAppId: app.id, isAppDrawerOpen: false, isRecentAppsOpen: false });
    } else {
      set({
        runningApps: [...runningApps, { ...app, openedAt: Date.now() }],
        currentAppId: app.id,
        isAppDrawerOpen: false,
        isRecentAppsOpen: false,
      });
    }
  },

  closeApp: (appId) => {
    const { runningApps, currentAppId } = get();
    const newRunningApps = runningApps.filter(app => app.id !== appId);
    const newCurrentApp = currentAppId === appId
      ? (newRunningApps.length > 0 ? newRunningApps[newRunningApps.length - 1].id : null)
      : currentAppId;

    set({
      runningApps: newRunningApps,
      currentAppId: newCurrentApp,
    });
  },

  switchToApp: (appId) => set({ currentAppId: appId, isRecentAppsOpen: false }),
  toggleAppDrawer: () => set((state) => ({ isAppDrawerOpen: !state.isAppDrawerOpen, isRecentAppsOpen: false })),
  toggleRecentApps: () => set((state) => ({ isRecentAppsOpen: !state.isRecentAppsOpen, isAppDrawerOpen: false })),
  closeAllOverlays: () => set({ isAppDrawerOpen: false, isRecentAppsOpen: false, currentAppId: null }),
}));

interface PersistentStore {
  homeScreenApps: string[];
  appPositions: { [key: string]: Position };
  widgetSettings: { [key: string]: { hide?: boolean; position?: Position; size?: Size } };

  addToHomeScreen: (appId: string) => void;
  removeFromHomeScreen: (appId: string) => void;
  moveItem: (appId: string, position: Position) => void;
  toggleWidget: (appId: string) => void;
  setWidgetPosition: (appId: string, position: Position) => void;
  setWidgetSize: (appId: string, size: Size) => void;
}

const usePersistentStore = create<PersistentStore>()(
  persist(
    (set) => ({
      homeScreenApps: [],
      appPositions: {},
      widgetSettings: {},

      addToHomeScreen: (appId) => {
        set((state) => ({
          homeScreenApps: [...state.homeScreenApps, appId],
          // Default position for apps at bottom of screen
          appPositions: {
            ...state.appPositions,
            [appId]: {
              x: Math.random() * (window.innerWidth - 100),
              y: window.innerHeight - 200 - Math.random() * 100
            }
          },
        }));
      },

      removeFromHomeScreen: (appId) => {
        set((state) => {
          const newPositions = { ...state.appPositions };
          delete newPositions[appId];
          return {
            homeScreenApps: state.homeScreenApps.filter(id => id !== appId),
            appPositions: newPositions,
          };
        });
      },

      moveItem: (appId, position) => {
        set((state) => ({
          appPositions: { ...state.appPositions, [appId]: position },
        }));
      },

      toggleWidget: (appId) => {
        set((state) => ({
          widgetSettings: {
            ...state.widgetSettings,
            [appId]: {
              ...state.widgetSettings[appId],
              hide: !state.widgetSettings[appId]?.hide,
              // Default position for widgets at top of screen
              position: state.widgetSettings[appId]?.position || {
                x: Math.random() * (window.innerWidth - 300),
                y: 50 + Math.random() * 100
              },
              // Default size
              size: state.widgetSettings[appId]?.size || { width: 300, height: 200 }
            },
          },
        }));
      },

      setWidgetPosition: (appId, position) => {
        set((state) => ({
          widgetSettings: {
            ...state.widgetSettings,
            [appId]: {
              ...state.widgetSettings[appId],
              position,
            },
          },
        }));
      },

      setWidgetSize: (appId, size) => {
        set((state) => ({
          widgetSettings: {
            ...state.widgetSettings,
            [appId]: {
              ...state.widgetSettings[appId],
              size,
            },
          },
        }));
      },
    }),
    {
      name: 'android-homescreen-store',
      storage: createJSONStorage(() => localStorage),
    }
  )
);

// ==================== COMPONENTS ====================

// Draggable wrapper component
const Draggable: React.FC<{
  id: string;
  position: Position;
  onMove: (position: Position) => void;
  isEditMode: boolean;
  children: React.ReactNode;
  className?: string;
}> = ({ position, onMove, isEditMode, children, className = '' }) => {
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const elementRef = useRef<HTMLDivElement>(null);

  const handleStart = (clientX: number, clientY: number) => {
    if (!isEditMode) return;
    setIsDragging(true);
    setDragOffset({
      x: clientX - position.x,
      y: clientY - position.y,
    });
  };

  const handleMove = (clientX: number, clientY: number) => {
    if (!isDragging) return;
    const newX = Math.max(0, Math.min(window.innerWidth - 100, clientX - dragOffset.x));
    const newY = Math.max(40, Math.min(window.innerHeight - 100, clientY - dragOffset.y));
    onMove({ x: newX, y: newY });
  };

  const handleEnd = () => {
    setIsDragging(false);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => handleMove(e.clientX, e.clientY);
    const handleTouchMove = (e: TouchEvent) => {
      const touch = e.touches[0];
      handleMove(touch.clientX, touch.clientY);
    };
    const handleMouseUp = () => handleEnd();
    const handleTouchEnd = () => handleEnd();

    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('touchmove', handleTouchMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.addEventListener('touchend', handleTouchEnd);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.removeEventListener('touchend', handleTouchEnd);
    };
  }, [isDragging, dragOffset, handleMove]);

  return (
    <div
      ref={elementRef}
      className={`absolute ${isDragging ? 'z-50' : ''} ${className}`}
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
        cursor: isEditMode ? 'move' : 'default',
        touchAction: isEditMode ? 'none' : 'auto',
      }}
      onMouseDown={(e) => handleStart(e.clientX, e.clientY)}
      onTouchStart={(e) => {
        const touch = e.touches[0];
        handleStart(touch.clientX, touch.clientY);
      }}
    >
      {children}
    </div>
  );
};

// App Icon Component
const AppIcon: React.FC<{
  app: HomepageApp;
  isEditMode: boolean;
  showLabel?: boolean;
  isFloating?: boolean;
}> = ({ app, isEditMode, showLabel = true, isFloating = false }) => {
  const { openApp } = useNavigationStore();
  const { removeFromHomeScreen } = usePersistentStore();
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
          className="absolute -top-2 -right-2 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center text-xs z-10 shadow-lg"
        >
          ×
        </button>
      )}

      <div className="w-16 h-16 mb-1 rounded-2xl overflow-hidden bg-gradient-to-br from-blue-400 to-blue-600 dark:from-blue-600 dark:to-blue-800 flex items-center justify-center shadow-lg">
        {app.base64_icon ? (
          <img src={app.base64_icon} alt={app.label} className="w-full h-full object-cover" />
        ) : (
          <div className="text-2xl text-white font-bold">{app.label[0]}</div>
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

// Widget Component
const Widget: React.FC<{ app: HomepageApp }> = ({ app }) => {
  const { toggleWidget, widgetSettings, setWidgetPosition, setWidgetSize } = usePersistentStore();
  const { isEditMode } = useHomepageStore();
  const [isLoading, setIsLoading] = useState(true);
  const [hasError, setHasError] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const resizeRef = useRef<HTMLDivElement>(null);

  const settings = widgetSettings[app.id] || {};
  if (settings.hide) return null;

  const position = settings.position || { x: 50, y: 50 };
  const size = settings.size || { width: 300, height: 200 };

  // Widgets can either have widget HTML content or be loaded from their app URL
  const isHtmlWidget = app.widget && app.widget !== 'true' && app.widget.includes('<');

  const handleError = () => {
    setHasError(true);
    setIsLoading(false);
  };

  const handleResize = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isEditMode) return;

    const startX = e.clientX;
    const startY = e.clientY;
    const startWidth = size.width;
    const startHeight = size.height;

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.max(200, startWidth + e.clientX - startX);
      const newHeight = Math.max(150, startHeight + e.clientY - startY);
      setWidgetSize(app.id, { width: newWidth, height: newHeight });
    };

    const handleMouseUp = () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      setIsResizing(false);
    };

    setIsResizing(true);
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  return (
    <Draggable
      id={app.id}
      position={position}
      onMove={(pos) => setWidgetPosition(app.id, pos)}
      isEditMode={isEditMode}
    >
      <div
        className={`bg-black/80 backdrop-blur-xl rounded-2xl overflow-hidden shadow-2xl border border-white/20
          ${isEditMode ? 'ring-2 ring-blue-400' : ''}
          ${isResizing ? 'pointer-events-none' : ''}`}
        style={{ width: `${size.width}px`, height: `${size.height}px` }}
      >
        <div className="flex items-center justify-between bg-gradient-to-r from-blue-500/20 to-purple-500/20 px-3 py-2 border-b border-white/10">
          <span className="text-white/90 text-sm font-medium">{app.label}</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              toggleWidget(app.id);
            }}
            className="text-white/60 hover:text-white transition-colors"
          >
            ×
          </button>
        </div>

        <div className="relative w-full h-[calc(100%-40px)]">
          {isLoading && !isHtmlWidget && !hasError && (
            <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/50">
              <div className="text-white/70 animate-pulse">
                <div className="w-8 h-8 border-2 border-white/30 border-t-white/70 rounded-full animate-spin mb-2"></div>
                <div className="text-sm">Loading...</div>
              </div>
            </div>
          )}

          {hasError ? (
            <div className="flex flex-col items-center justify-center h-full text-white/50 text-center p-4">
              <div className="text-3xl mb-2">⚠️</div>
              <div className="text-sm">Failed to load widget</div>
            </div>
          ) : isHtmlWidget ? (
            <iframe
              srcDoc={app.widget}
              className="w-full h-full bg-white"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
              onLoad={() => setIsLoading(false)}
              onError={handleError}
            />
          ) : (
            <iframe
              src={app.path || `/app:${app.process}:${app.publisher}.os/`}
              className="w-full h-full bg-white"
              onLoad={() => setIsLoading(false)}
              onError={handleError}
              // Enhanced permissions for CORS
              allow="accelerometer; camera; encrypted-media; geolocation; gyroscope; microphone; midi; payment; usb; xr-spatial-tracking"
              // Minimal sandbox for widget functionality
              sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals allow-downloads allow-presentation allow-top-navigation-by-user-activation"
            />
          )}
        </div>

        {isEditMode && (
          <div
            ref={resizeRef}
            className="absolute bottom-0 right-0 w-4 h-4 bg-blue-400 cursor-se-resize rounded-tl-lg"
            onMouseDown={handleResize}
          />
        )}
      </div>
    </Draggable>
  );
};

// Gesture Zone Component
const GestureZone: React.FC = () => {
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
        className={`fixed right-0 top-0 w-8 h-full z-40 transition-all cursor-pointer
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

// App Drawer Component
const AppDrawer: React.FC = () => {
  const { apps } = useHomepageStore();
  const { isAppDrawerOpen, toggleAppDrawer } = useNavigationStore();
  const { homeScreenApps, addToHomeScreen } = usePersistentStore();
  const [searchQuery, setSearchQuery] = useState('');

  const filteredApps = useMemo(() => {
    return apps
      .filter(app => app.label.toLowerCase().includes(searchQuery.toLowerCase()))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [apps, searchQuery]);

  const handleAddToHome = (app: HomepageApp) => {
    // Ensure we don't duplicate
    if (!homeScreenApps.includes(app.id)) {
      addToHomeScreen(app.id);
    }
    toggleAppDrawer();
  };

  if (!isAppDrawerOpen) return null;

  return (
    <div className="fixed inset-0 bg-gradient-to-b from-gray-900/98 to-black/98 backdrop-blur-xl z-50 flex flex-col">
      <div className="p-4">
        <div className="relative">
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-4 py-3 pl-12 bg-white/10 backdrop-blur rounded-2xl text-white placeholder-white/50 border border-white/20 focus:border-blue-400 focus:outline-none transition-all"
            autoFocus
          />
          <div className="absolute left-4 top-1/2 transform -translate-y-1/2 text-white/50">
            🔍
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-4 md:grid-cols-6 gap-4">
          {filteredApps.map(app => (
            <div key={app.id} className="relative group">
              <AppIcon app={app} isEditMode={false} />
              {!homeScreenApps.includes(app.id) && (
                <button
                  onClick={() => handleAddToHome(app)}
                  className="absolute -top-2 -right-2 w-7 h-7 bg-green-500 text-white rounded-full flex items-center justify-center text-sm shadow-lg opacity-0 group-hover:opacity-100 transition-opacity"
                >
                  +
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      <button
        onClick={toggleAppDrawer}
        className="p-6 text-white/70 text-center hover:text-white transition-colors"
      >
        Close
      </button>
    </div>
  );
};

// Recent Apps Component
const RecentApps: React.FC = () => {
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

// App Container Component
const AppContainer: React.FC<{ app: RunningApp; isVisible: boolean }> = ({ app, isVisible }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [hasError, setHasError] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  // Ensure we have a valid path
  const appUrl = app.path || `/app:${app.process}:${app.publisher}.os/`;

  const handleError = () => {
    setHasError(true);
    setIsLoading(false);
    console.error(`Failed to load app: ${app.label}`);
  };

  const handleLoad = () => {
    setIsLoading(false);

    // Handle subdomain redirects
    try {
      const iframe = iframeRef.current;
      if (iframe && iframe.contentWindow) {
        // Check if we need to redirect to subdomain
        const currentHost = window.location.host;
        const expectedSubdomain = generateSubdomain(app.process, app.publisher);

        if (!currentHost.startsWith(expectedSubdomain)) {
          // Redirect to subdomain version
          const protocol = window.location.protocol;
          const port = window.location.port ? `:${window.location.port}` : '';
          const baseDomain = currentHost.split('.').slice(1).join('.');
          const subdomainUrl = `${protocol}//${expectedSubdomain}.${baseDomain}${port}${appUrl}`;

          // Use window.location for redirect to handle authentication
          window.location.href = subdomainUrl;
        }
      }
    } catch (e) {
      // Iframe might be cross-origin, that's ok
    }
  };

  const generateSubdomain = (process: string, publisher: string) => {
    return `${process}-${publisher}`.toLowerCase()
      .split('')
      .map(c => c.match(/[a-zA-Z0-9]/) ? c : '-')
      .join('');
  };

  return (
    <div
      className={`fixed inset-0 bg-white z-30 transition-transform duration-300
        ${isVisible ? 'translate-x-0' : 'translate-x-full'}`}
    >
      {hasError ? (
        <div className="w-full h-full flex items-center justify-center bg-gradient-to-b from-gray-100 to-gray-200 dark:from-gray-800 dark:to-gray-900">
          <div className="text-center">
            <div className="text-6xl mb-4">⚠️</div>
            <h2 className="text-xl font-semibold mb-2 text-gray-800 dark:text-gray-200">Failed to load {app.label}</h2>
            <p className="text-gray-600 dark:text-gray-400">The app could not be loaded.</p>
          </div>
        </div>
      ) : (
        <>
          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-b from-gray-100 to-gray-200 dark:from-gray-800 dark:to-gray-900 z-10">
              <div className="text-center">
                <div className="w-12 h-12 border-4 border-gray-300 dark:border-gray-600 border-t-blue-500 rounded-full animate-spin mb-4"></div>
                <p className="text-gray-600 dark:text-gray-400">Loading {app.label}...</p>
              </div>
            </div>
          )}
          <iframe
            ref={iframeRef}
            src={appUrl}
            className="w-full h-full border-0"
            title={app.label}
            onError={handleError}
            onLoad={handleLoad}
            // Allow all necessary permissions for subdomain redirects
            allow="accelerometer; camera; encrypted-media; geolocation; gyroscope; microphone; midi; payment; usb; xr-spatial-tracking"
            // Enhanced sandbox permissions
            sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation allow-modals allow-downloads allow-presentation allow-storage-access-by-user-activation"
          />
        </>
      )}
    </div>
  );
};

// Home Screen Component
const HomeScreen: React.FC = () => {
  const { apps } = useHomepageStore();
  const { homeScreenApps, appPositions, widgetSettings, toggleWidget, moveItem } = usePersistentStore();
  const { isEditMode, setEditMode } = useHomepageStore();
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

// Main App Component
export default function AndroidHomescreen() {
  const { setApps } = useHomepageStore();
  const { runningApps, currentAppId, isAppDrawerOpen, isRecentAppsOpen, toggleRecentApps, switchToApp, toggleAppDrawer, closeAllOverlays } = useNavigationStore();
  const [loading, setLoading] = useState(true);

  // Keyboard shortcuts for desktop
  useEffect(() => {
    const handleKeyPress = (e: KeyboardEvent) => {
      // Ignore if user is typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      // Single key shortcuts
      switch(e.key.toLowerCase()) {
        case 'a':
          e.preventDefault();
          if (!isAppDrawerOpen) toggleAppDrawer();
          break;
        case 's':
          e.preventDefault();
          if (!isRecentAppsOpen) toggleRecentApps();
          break;
        case 'h':
          e.preventDefault();
          closeAllOverlays();
          break;
        case 'escape':
          e.preventDefault();
          closeAllOverlays();
          break;
      }

      // Number keys to switch apps
      if (e.key >= '1' && e.key <= '9') {
        const index = parseInt(e.key) - 1;
        if (runningApps[index]) {
          e.preventDefault();
          switchToApp(runningApps[index].id);
        }
      }
    };

    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  }, [runningApps, isRecentAppsOpen, isAppDrawerOpen, toggleRecentApps, toggleAppDrawer, switchToApp, closeAllOverlays]);

  // Fetch apps from backend
  useEffect(() => {
    fetch('/apps', { credentials: 'include' })
      .then(res => res.json())
      .then(data => {
        setApps(data);
        setLoading(false);
      })
      .catch((error) => {
        console.warn('Failed to fetch apps from backend:', error);
        // Fallback demo apps for development
        setApps([
          { id: '1', process: 'settings', package_name: 'settings', publisher: 'sys', path: '/app:settings:sys.os/', label: 'Settings', order: 1, favorite: true },
          { id: '2', process: 'files', package_name: 'files', publisher: 'sys', path: '/app:files:sys.os/', label: 'Files', order: 2, favorite: false },
          { id: '3', process: 'terminal', package_name: 'terminal', publisher: 'sys', path: '/app:terminal:sys.os/', label: 'Terminal', order: 3, favorite: false },
          { id: '4', process: 'browser', package_name: 'browser', publisher: 'sys', path: '/app:browser:sys.os/', label: 'Browser', order: 4, favorite: true },
          { id: '5', process: 'app-store', package_name: 'app-store', publisher: 'sys', path: '/main:app-store:sys/', label: 'App Store', order: 5, favorite: false, widget: 'true' },
        ]);
        setLoading(false);
      });
  }, [setApps]);

  if (loading) {
    return (
      <div className="fixed inset-0 bg-gradient-to-br from-gray-900 to-black flex items-center justify-center">
        <div className="text-center">
          <div className="w-16 h-16 border-4 border-gray-700 border-t-blue-500 rounded-full animate-spin mb-4"></div>
          <div className="text-gray-300 text-xl">Loading Hyperware...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-black overflow-hidden">
      <style>
        {`
          @keyframes wiggle {
            0%, 100% { transform: rotate(-3deg); }
            50% { transform: rotate(3deg); }
          }

          .animate-wiggle {
            animation: wiggle 0.3s ease-in-out infinite;
          }
        `}
      </style>

      {/* Home Screen */}
      <HomeScreen />

      {/* Running Apps */}
      {runningApps.map(app => (
        <AppContainer
          key={app.id}
          app={app}
          isVisible={currentAppId === app.id && !isAppDrawerOpen && !isRecentAppsOpen}
        />
      ))}

      {/* Overlays */}
      <AppDrawer />
      <RecentApps />

      {/* Gesture Zone */}
      <GestureZone />
    </div>
  );
}
