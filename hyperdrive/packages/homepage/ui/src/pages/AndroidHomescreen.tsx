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
  appPositions: { [key: string]: { page: number; position: number } };
  widgetSettings: { [key: string]: { hide?: boolean; size?: 'small' | 'large' } };

  addToHomeScreen: (appId: string, page: number, position: number) => void;
  removeFromHomeScreen: (appId: string) => void;
  moveApp: (appId: string, page: number, position: number) => void;
  toggleWidget: (appId: string) => void;
}

const usePersistentStore = create<PersistentStore>()(
  persist(
    (set) => ({
      homeScreenApps: [],
      appPositions: {},
      widgetSettings: {},

      addToHomeScreen: (appId, page, position) => {
        set((state) => ({
          homeScreenApps: [...state.homeScreenApps, appId],
          appPositions: { ...state.appPositions, [appId]: { page, position } },
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

      moveApp: (appId, page, position) => {
        set((state) => ({
          appPositions: { ...state.appPositions, [appId]: { page, position } },
        }));
      },

      toggleWidget: (appId) => {
        set((state) => ({
          widgetSettings: {
            ...state.widgetSettings,
            [appId]: {
              ...state.widgetSettings[appId],
              hide: !state.widgetSettings[appId]?.hide,
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

// App Icon Component
const AppIcon: React.FC<{
  app: HomepageApp;
  isEditMode: boolean;
  showLabel?: boolean;
}> = ({ app, isEditMode, showLabel = true }) => {
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
        ${isEditMode ? 'animate-wiggle' : 'active:scale-95'}
        ${!app.path && !(app.process && app.publisher) ? 'opacity-50' : ''}`}
      onMouseDown={() => setIsPressed(true)}
      onMouseUp={() => setIsPressed(false)}
      onMouseLeave={() => setIsPressed(false)}
      onClick={handlePress}
    >
      {isEditMode && (
        <button
          onClick={handleRemove}
          className="absolute -top-1 -right-1 w-6 h-6 bg-red-500 text-white rounded-full flex items-center justify-center text-xs z-10"
        >
          ×
        </button>
      )}

      <div className="w-16 h-16 mb-1 rounded-2xl overflow-hidden bg-gray-200 dark:bg-gray-700 flex items-center justify-center">
        {app.base64_icon ? (
          <img src={app.base64_icon} alt={app.label} className="w-full h-full object-cover" />
        ) : (
          <div className="text-2xl">{app.label[0]}</div>
        )}
      </div>

      {showLabel && (
        <span className="text-xs text-center max-w-full truncate text-white">
          {app.label}
        </span>
      )}
    </div>
  );
};

// Widget Component
const Widget: React.FC<{ app: HomepageApp }> = ({ app }) => {
  const { toggleWidget, widgetSettings } = usePersistentStore();
  const [isLoading, setIsLoading] = useState(true);
  const [hasError, setHasError] = useState(false);

  if (widgetSettings[app.id]?.hide) return null;

  // Widgets can either have widget HTML content or be loaded from their app URL
  const isHtmlWidget = app.widget && app.widget !== 'true' && app.widget.includes('<');

  const handleError = () => {
    setHasError(true);
    setIsLoading(false);
  };

  return (
    <div className="relative bg-white/10 backdrop-blur-sm rounded-2xl overflow-hidden col-span-2 row-span-2">
      <div className="absolute top-2 right-2 z-10 flex gap-2">
        <span className="text-white/50 text-xs">{app.label}</span>
        <button
          onClick={() => toggleWidget(app.id)}
          className="text-white/50 hover:text-white"
        >
          ×
        </button>
      </div>

      {isLoading && !isHtmlWidget && !hasError && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-white/5">
          <div className="text-white/50 animate-pulse text-center">
            <div className="text-2xl mb-2">⏳</div>
            <div className="text-sm">Loading {app.label}...</div>
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
          className="w-full h-full"
          sandbox="allow-scripts"
          onLoad={() => setIsLoading(false)}
          onError={handleError}
        />
      ) : (
        <iframe
          src={app.path || `/app:${app.process}:${app.publisher}.os/`}
          className="w-full h-full"
          onLoad={() => setIsLoading(false)}
          onError={handleError}
          // Allow necessary permissions for widgets
          allow="accelerometer; camera; encrypted-media; geolocation; gyroscope; microphone; midi; payment; usb"
          // Minimal sandbox for widget functionality
          sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals"
        />
      )}
    </div>
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
          ${isHovered && !isActive ? 'bg-white/10' : ''}`}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onClick={handleClick}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      />
      {/* Desktop hint */}
      {isHovered && !isActive && (
        <div className="fixed right-12 top-1/2 transform -translate-y-1/2 bg-black/80 text-white px-3 py-2 rounded-lg text-sm pointer-events-none z-50">
          <div>Click or press <kbd className="px-1 bg-white/20 rounded">S</kbd> for recent apps</div>
          <div>Press <kbd className="px-1 bg-white/20 rounded">A</kbd> for all apps</div>
          <div>Press <kbd className="px-1 bg-white/20 rounded">H</kbd> for home</div>
        </div>
      )}
    </>
  );
};

// App Drawer Component
const AppDrawer: React.FC = () => {
  const { apps } = useHomepageStore();
  const { isAppDrawerOpen, toggleAppDrawer } = useNavigationStore();
  const { homeScreenApps, addToHomeScreen, appPositions } = usePersistentStore();
  const [searchQuery, setSearchQuery] = useState('');

  const filteredApps = useMemo(() => {
    return apps
      .filter(app => app.label.toLowerCase().includes(searchQuery.toLowerCase()))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [apps, searchQuery]);

  const handleAddToHome = (app: HomepageApp) => {
    // Find first empty position, ensuring we don't duplicate
    if (!homeScreenApps.includes(app.id)) {
      // Add to page 0 at the next available position
      const existingPositions = Object.values(appPositions)
        .filter(pos => pos.page === 0)
        .map(pos => pos.position);
      const nextPosition = existingPositions.length > 0
        ? Math.max(...existingPositions) + 1
        : 0;
      addToHomeScreen(app.id, 0, nextPosition);
    }
    toggleAppDrawer();
  };

  if (!isAppDrawerOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/95 backdrop-blur-md z-50 flex flex-col">
      <div className="p-4">
        <input
          type="text"
          placeholder="Search apps..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full px-4 py-2 bg-white/10 backdrop-blur rounded-full text-white placeholder-white/50"
        />
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-4 gap-4">
          {filteredApps.map(app => (
            <div key={app.id} className="relative">
              <AppIcon app={app} isEditMode={false} />
              {!homeScreenApps.includes(app.id) && (
                <button
                  onClick={() => handleAddToHome(app)}
                  className="absolute -top-1 -right-1 w-6 h-6 bg-green-500 text-white rounded-full flex items-center justify-center text-xs"
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
        className="p-4 text-white text-center"
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
    <div className="fixed inset-0 bg-black/95 backdrop-blur-md z-50 flex items-center justify-center">
      {runningApps.length === 0 ? (
        <div className="text-center">
          <div className="text-6xl mb-4 text-white/30">📱</div>
          <h2 className="text-xl text-white/70 mb-2">No running apps</h2>
          <p className="text-white/50 mb-8">Open an app to see it here</p>
          <button
            onClick={closeAllOverlays}
            className="px-6 py-2 bg-white/10 backdrop-blur rounded-full text-white hover:bg-white/20 transition-colors"
          >
            🏠 Back to Home
          </button>
        </div>
      ) : (
        <>
          <div className="w-full max-w-4xl h-96 overflow-x-auto">
            <div className="flex gap-4 p-4 h-full items-center">
              {runningApps.map(app => (
                <div
                  key={app.id}
                  className="relative flex-shrink-0 w-64 h-full bg-gray-800 rounded-2xl overflow-hidden cursor-pointer group"
                  onClick={() => switchToApp(app.id)}
                >
                  <div className="p-4 bg-gray-900 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      {app.base64_icon && (
                        <img src={app.base64_icon} alt={app.label} className="w-8 h-8 rounded" />
                      )}
                      <span className="text-white text-sm">{app.label}</span>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        closeApp(app.id);
                      }}
                      className="text-white/50 hover:text-white"
                    >
                      ×
                    </button>
                  </div>

                  <div className="p-4 text-white/50 text-center">
                    <div className="text-6xl mb-2">⧉</div>
                    <p className="text-sm">App Preview</p>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="absolute bottom-8 left-1/2 transform -translate-x-1/2 flex gap-4">
            <button
              onClick={closeAllOverlays}
              className="px-6 py-2 bg-white/10 backdrop-blur rounded-full text-white hover:bg-white/20 transition-colors"
            >
              🏠 Home
            </button>
            <button
              onClick={toggleRecentApps}
              className="px-6 py-2 bg-white/10 backdrop-blur rounded-full text-white hover:bg-white/20 transition-colors"
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
    // The iframe will handle redirects automatically
  };

  return (
    <div
      className={`fixed inset-0 bg-white z-30 transition-transform duration-300
        ${isVisible ? 'translate-x-0' : 'translate-x-full'}`}
    >
      {hasError ? (
        <div className="w-full h-full flex items-center justify-center bg-gray-100">
          <div className="text-center">
            <div className="text-6xl mb-4">⚠️</div>
            <h2 className="text-xl font-semibold mb-2">Failed to load {app.label}</h2>
            <p className="text-gray-600">The app could not be loaded.</p>
          </div>
        </div>
      ) : (
        <>
          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center bg-gray-100 z-10">
              <div className="text-center">
                <div className="text-4xl mb-2 animate-spin">⏳</div>
                <p>Loading {app.label}...</p>
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
            // Minimal sandbox restrictions to allow redirects and full functionality
            sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation allow-modals allow-downloads"
          />
        </>
      )}
    </div>
  );
};

// Home Screen Component
const HomeScreen: React.FC = () => {
  const { apps } = useHomepageStore();
  const { homeScreenApps, appPositions, widgetSettings, toggleWidget } = usePersistentStore();
  const { isEditMode, setEditMode } = useHomepageStore();
  const { toggleAppDrawer } = useNavigationStore();
  const [currentPage] = useState(0);

  const homeApps = useMemo(() => {
    return apps.filter(app => homeScreenApps.includes(app.id));
  }, [apps, homeScreenApps]);

  const pageApps = useMemo(() => {
    return homeApps.filter(app => {
      const position = appPositions[app.id];
      return position && position.page === currentPage;
    });
  }, [homeApps, appPositions, currentPage]);

  const dockApps = useMemo(() => {
    // Get first 4 favorited apps for the dock, or first 4 apps if no favorites
    const favoriteApps = homeApps.filter(app => app.favorite).slice(0, 4);
    return favoriteApps.length > 0 ? favoriteApps : homeApps.slice(0, 4);
  }, [homeApps]);

  const nonDockPageApps = useMemo(() => {
    // Filter out dock apps from page apps to avoid duplicates
    const dockAppIds = dockApps.map(app => app.id);
    return pageApps.filter(app => !dockAppIds.includes(app.id));
  }, [pageApps, dockApps]);

  const widgetApps = useMemo(() => {
    return homeApps.filter(app => app.widget && !widgetSettings[app.id]?.hide);
  }, [homeApps, widgetSettings]);

  return (
    <div className="flex-1 relative bg-gradient-to-b from-blue-900 to-black">
      {/* Wallpaper overlay */}
      <div className="absolute inset-0 bg-black/30" />

      {/* Content */}
      <div className="relative z-10 h-full flex flex-col p-4">
        {/* Status bar placeholder */}
        <div className="h-8 mb-4" />

        {/* Widgets area */}
        <div className="mb-4">
          {widgetApps.slice(0, 1).map(app => (
            <Widget key={app.id} app={app} />
          ))}
        </div>

        {/* Apps grid */}
        <div className="flex-1">
          <div className="grid grid-cols-4 gap-4 auto-rows-min">
            {nonDockPageApps.map(app => (
              <AppIcon key={app.id} app={app} isEditMode={isEditMode} />
            ))}
          </div>
        </div>

        {/* Dock */}
        <div className="h-24 bg-black/30 backdrop-blur-sm rounded-2xl p-2 flex items-center justify-around">
          {dockApps.map(app => (
            <AppIcon key={app.id} app={app} isEditMode={isEditMode} showLabel={false} />
          ))}
          <button
            onClick={toggleAppDrawer}
            className="w-16 h-16 bg-white/10 backdrop-blur rounded-2xl flex items-center justify-center text-white text-2xl"
          >
            ⊞
          </button>
        </div>

        {/* Edit mode toggle and widget settings */}
        {!isEditMode && (
          <button
            onClick={() => setEditMode(true)}
            className="absolute top-4 right-4 px-3 py-1 bg-white/10 backdrop-blur rounded-full text-white text-sm"
          >
            Edit
          </button>
        )}

        {isEditMode && (
          <>
            <button
              onClick={() => setEditMode(false)}
              className="absolute top-4 right-4 px-3 py-1 bg-green-500 rounded-full text-white text-sm"
            >
              Done
            </button>

            {/* Widget management in edit mode */}
            <div className="absolute top-16 right-4 bg-black/80 backdrop-blur rounded-lg p-3 max-w-xs">
              <h3 className="text-white text-sm font-semibold mb-2">Widgets</h3>
              <div className="space-y-2 max-h-64 overflow-y-auto">
                {homeApps.filter(app => app.widget).map(app => (
                  <div key={app.id} className="flex items-center justify-between text-white/80 text-sm">
                    <span>{app.label}</span>
                    <button
                      onClick={() => toggleWidget(app.id)}
                      className={`px-2 py-1 rounded text-xs ${
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
                  <p className="text-white/50 text-sm">No apps with widgets on home screen</p>
                )}
              </div>
            </div>
          </>
        )}

        {/* Desktop hint */}
        <div className="hidden md:block absolute bottom-32 left-4 text-white/30 text-xs">
          <kbd className="px-1 bg-white/10 rounded">A</kbd> All apps •
          <kbd className="px-1 bg-white/10 rounded">S</kbd> Recent apps •
          <kbd className="px-1 bg-white/10 rounded">H</kbd> Home •
          <kbd className="px-1 bg-white/10 rounded">1-9</kbd> Switch apps
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
      <div className="fixed inset-0 bg-gray-100 dark:bg-gray-900 flex items-center justify-center">
        <div className="text-center">
          <div className="text-4xl mb-2 animate-spin text-gray-600 dark:text-gray-300">⏳</div>
          <div className="text-gray-800 dark:text-gray-200 text-xl">Loading...</div>
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
