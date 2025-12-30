import React, { useMemo, useEffect } from 'react';
import { useAppStore } from '../../../stores/appStore';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { useNavigationStore } from '../../../stores/navigationStore';
import { useNotificationStore } from '../../../stores/notificationStore';
import { Draggable } from './Draggable';
import { AppIcon } from './AppIcon';
import { Widget } from './Widget';
import { NotificationMenu } from './NotificationMenu';
import type { HomepageApp } from '../../../types/app.types';
import { BsBell, BsCheck, BsClock, BsEnvelope, BsGridFill, BsImage, BsLayers, BsPencilSquare, BsSearch, BsX } from 'react-icons/bs';
import classNames from 'classnames';
import { Modal } from './Modal';
import { initializePushNotifications } from '../../../utils/pushNotifications';

export const HomeScreen: React.FC = () => {
  const { apps } = useAppStore();
  const {
    homeScreenApps,
    dockApps,
    appPositions,
    widgetSettings,
    removeFromHomeScreen,
    toggleWidget,
    moveItem,
    backgroundImage,
    setBackgroundImage,
    addToDock,
    removeFromDock,
    isInitialized,
    setIsInitialized,
    addToHomeScreen,
    doNotShowOnboardingAgain,
    setDoNotShowOnboardingAgain,
  } = usePersistenceStore();
  const { isEditMode, setEditMode } = useAppStore();
  const { openApp, toggleAppDrawer, toggleRecentApps } = useNavigationStore();
  const {
    getUnreadCount,
    menuOpen,
    setMenuOpen,
    setPermissionGranted,
    hasVapidKey,
  } = useNotificationStore();
  const [draggedAppId, setDraggedAppId] = React.useState<string | null>(null);
  const [touchDragPosition, setTouchDragPosition] = React.useState<{ x: number; y: number } | null>(null);
  const [showBackgroundSettings, setShowBackgroundSettings] = React.useState(false);
  const [showWidgetSettings, setShowWidgetSettings] = React.useState(false);
  const [showOnboarding, setShowOnboarding] = React.useState(!doNotShowOnboardingAgain);
  const [showWidgetOnboarding, setShowWidgetOnboarding] = React.useState(!doNotShowOnboardingAgain);
  const unreadCount = getUnreadCount();
  const showNotificationIndicator = unreadCount > 0 || hasVapidKey === false;

  const handleNotificationClick = async () => {
    let shouldInitializeNotifications = false;

    if ('Notification' in window && 'serviceWorker' in navigator) {
      const currentPermission = Notification.permission;

      if (currentPermission === 'default') {
        const permissionResult = await Notification.requestPermission();
        const granted = permissionResult === 'granted';
        setPermissionGranted(granted);
        shouldInitializeNotifications = granted;
      } else {
        const granted = currentPermission === 'granted';
        setPermissionGranted(granted);
        shouldInitializeNotifications = granted;
      }

      if (shouldInitializeNotifications) {
        try {
          const registration = await navigator.serviceWorker.ready;
          await initializePushNotifications(registration);
        } catch (error) {
          console.error('[Push] Failed to initialize notifications:', error);
        }
      }
    }

    setMenuOpen(!menuOpen);
  };

  useEffect(() => {
    if (isInitialized) return;
    // add appstore, contacts, and settings to the homepage on initial load
    setIsInitialized(true);
    // default widgets
    addToHomeScreen("main:app-store:sys");
    addToHomeScreen("contacts:contacts:sys");
    addToHomeScreen("settings:settings:sys");
    addToHomeScreen("homepage:homepage:sys"); // actually the clock widget

    // disable these widgets by default
    toggleWidget("main:app-store:sys");
    toggleWidget("settings:settings:sys");

    // default dock apps
    addToDock("settings:settings:sys", 0);
    addToDock("main:app-store:sys", 1);

    setBackgroundImage('/large-background-vector.svg');
  }, [isInitialized]);

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
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
    const appId = e.dataTransfer.getData('appId');
    if (appId) {
      // Add to dock at the specified index
      // The addToDock function handles removing from existing position if needed
      addToDock(appId, index);
    }
  };

  const handleDockDragOver = (e: React.DragEvent) => {
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
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
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
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
    return homeApps.filter(app => app.widget);
  }, [homeApps]);

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

  const isDarkMode = useMemo(() => {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  }, []);

  const calculateAppIconPosition = (appId: string, index: number, totalApps: number) => {
    const isMobile = window.innerWidth < 768; // Tailwind md breakpoint
    const spacing = isMobile ? 5 : 10;
    const screenPortion = window.innerWidth / totalApps
    const dockHeight = 120;
    const iconHeight = 96;
    // if no setting, row along screen bottom above dock
    const y = window.innerHeight - dockHeight - spacing - iconHeight
    // space evenly
    const x = index * (screenPortion + spacing) + screenPortion / 4
    // ensure position sets so future movements do not cause a jump
    moveItem(appId, { x, y });
    return { x, y }
  }

  return (
    <div
      className="home-screen h-full w-full relative overflow-hidden"
      style={{
        backgroundColor: backgroundImage ? 'transparent' : isDarkMode ? '#111' : '#f6f6f6',
        backgroundImage: backgroundImage ? `url(${backgroundImage})` : 'none',
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundRepeat: 'no-repeat',
        touchAction: 'none'
      }}
      data-background-image={backgroundImage}
      data-is-dark-mode={isDarkMode}
    >

      {backgroundImage && (
        <div className="absolute inset-0 opacity-20" style={{
          backgroundImage: `url(${backgroundImage})`,
          backgroundSize: 'cover',
          backgroundPosition: 'center',
          backgroundRepeat: 'no-repeat',
        }} />
      )}


      <div
        className="relative z-10 h-full"
        onDragOver={(e) => {
          try { e.preventDefault(); } catch { }
          try { e.stopPropagation(); } catch { }
          e.dataTransfer.dropEffect = 'move';
        }}
        onDrop={(e) => {
          try { e.preventDefault(); } catch { }
          try { e.stopPropagation(); } catch { }
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
            try { e.preventDefault(); } catch { }
            try { e.stopPropagation(); } catch { }
          }
        }}
      >

        {floatingApps
          .filter(app => {
            return !app.id.includes('homepage:homepage:sys')  // don't show the clock icon because it does nothing.
            // && (!searchQuery || app.label.toLowerCase().includes(searchQuery.toLowerCase()))
          })
          .map((app, index, allApps) => {
            const position = appPositions[app.id] || calculateAppIconPosition(app.id, index, allApps.length);

            return (
              <Draggable
                key={app.id}
                id={app.id}
                position={position}
                onMove={(pos) => moveItem(app.id, pos)}
                isEditMode={true}
              >
                {isEditMode && (
                  <button
                    onClick={() => removeFromHomeScreen(app.id)}
                    className="absolute -top-2 -right-2 w-6 h-6 !p-0 !bg-red-500 !text-white !rounded-full  text-xs z-10 shadow-lg hover:!bg-red-600 transition-colors z-10"
                  >
                    <BsX />
                  </button>
                )}
                <div
                  onTouchStart={handleTouchStart(app.id)}
                  onTouchMove={handleTouchMove}
                  onTouchEnd={handleTouchEnd}
                >
                  <AppIcon
                    app={app}
                    isEditMode={isEditMode}
                    isFloating={true} />
                </div>
              </Draggable>
            );
          })}


        {widgetApps
          .map((app, index) => (
            <Widget
              key={app.id}
              app={app}
              index={index}
              totalWidgets={widgetApps.length}
              className={classNames({
                // 'invisible pointer-events-none': searchQuery && !app.label.toLowerCase().includes(searchQuery.toLowerCase()) && !widgetSettings[app.id]?.hide
                'invisible pointer-events-none': widgetSettings?.[app.id]?.hide
              })}
            >
              {showWidgetOnboarding && index === 0 && <div
                className="absolute bottom-2 left-1/2 -translate-x-1/2 px-2 py-1 rounded-lg bg-neon w-fit text-xs animate-pulse text-black cursor-pointer hover:opacity-80"
                onClick={() => setShowWidgetOnboarding(false)}
              >
                This is a widget. Drag it, resize it, or hide it!
              </div>}
            </Widget>
          ))}


        <div
          className="dock-area absolute bottom-0 md:bottom-4 left-1/2 transform -translate-x-1/2"
          onDragOver={handleDockDragOver}
          onDrop={(e) => handleDockDrop(e, dockAppsList.length)}
        >
          <div className="bg-white/10 backdrop-blur-xl rounded-t-3xl md:rounded-b-3xl p-3 flex  gap-2 shadow-2xl border-b-0 md:border-b border border-white/20">

            {Array.from({ length: 4 }).map((_, index) => {
              const app = dockAppsList[index];
              return (
                <div
                  key={`slot-${index}`}
                  data-dock-index={index}
                  className={classNames("w-14 md:w-16 relative", {
                    'h-14 md:h-16': !app,
                    'self-stretch': app,
                  })}
                  onDragOver={handleDockDragOver}
                  onDrop={(e) => {
                    try { e.preventDefault(); } catch { }
                    try { e.stopPropagation(); } catch { }
                    handleDockDrop(e, index);
                  }}
                >
                  {app ? (
                    <div
                      className="dock-icon"
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
                      <AppIcon
                        app={app}
                        isEditMode={isEditMode}
                        isUndocked={false}
                      />
                    </div>
                  ) : (
                    <div className="w-full h-full border-2 border-dashed border-black/20 dark:border-white/20 rounded-2xl transition-all hover:border-black/40 dark:hover:border-white/40 hover:bg-black/5 dark:hover:bg-white/5" />
                  )}
                </div>
              );
            })}
            <div className="w-px h-12 bg-black/20 dark:bg-white/20 mx-1 mt-1" />
            <div
              className="flex flex-col gap-1 items-center"
              onClick={toggleAppDrawer}
            >
              <button
                className=" w-14 h-14 md:w-16 md:h-16 !bg-iris !text-neon !rounded-xl text-2xl hover:!bg-neon hover:!text-iris "
              >
                <BsGridFill className="text-2xl" />
              </button>
              <span className="text-[10px] !text-black dark:!text-white whitespace-nowrap">My apps</span>
            </div>
            <div
              className="hidden md:flex flex-col gap-1 items-center"
              onClick={toggleRecentApps}
            >
              <button
                className="flex w-14 h-14 md:w-16 md:h-16 !bg-iris !text-neon !rounded-xl text-2xl hover:!bg-neon hover:!text-iris "
              >
                <BsClock className="text-2xl" />
              </button>
              <span className="text-[10px] !text-black dark:!text-white whitespace-nowrap">Recent</span>
            </div>
          </div>
        </div>


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
              isUndocked={false}
            />
          </div>
        )}


        <div className="absolute top-2 right-2 left-2 flex items-center gap-2 max-w-screen">
          <img
            src="/Logomark Iris.svg"
            alt="Hyperdrive"
            className="h-8 md:hidden self-start"
          />
          <img
            src="/Logo Iris.svg"
            alt="Hyperdrive"
            className="h-8 hidden md:block self-start"
          />

          {isEditMode && (
            <div className="flex flex-col items-end gap-2 grow">
              <div className={"flex items-center justify-end gap-2"}>
                <button
                  onClick={() => {
                    setShowBackgroundSettings(!showBackgroundSettings)
                    setShowWidgetSettings(false)
                  }}
                  className={classNames("!p-2 text-sm", {
                    'bg-gradient-to-r from-gray-600 to-gray-700 !text-neon': !showBackgroundSettings,
                    'bg-neon text-black': showBackgroundSettings,
                  })}
                  title="Settings"
                >
                  <BsImage />
                  <span>Background</span>
                </button>

                <button
                  onClick={() => {
                    setShowWidgetSettings(!showWidgetSettings)
                    setShowBackgroundSettings(false)
                  }}
                  className={classNames("!p-2 text-sm", {
                    'bg-gradient-to-r from-gray-600 to-gray-700 !text-neon': !showWidgetSettings,
                    'bg-neon text-black': showWidgetSettings,
                  })}
                  title="Settings"
                >
                  <BsLayers />
                  <span>Widgets</span>
                </button>
                <button
                  onClick={() => {
                    setEditMode(false);
                    setShowBackgroundSettings(false);
                    setShowWidgetSettings(false);
                  }}
                  className="!p-2 bg-gradient-to-r from-gray-600 to-gray-700 !text-neon text-sm"
                >
                  <BsCheck />
                  <span>Done</span>
                </button>
              </div>

              <div className={classNames("flex items-start gap-2", {
                hidden: !showBackgroundSettings && !showWidgetSettings
              })}>

                {showBackgroundSettings && (
                  <div className="bg-black/80 backdrop-blur-xl rounded-2xl p-4 shadow-2xl border border-white/20 z-100">
                    <span className="text-neon prose text-sm font-semibold mb-3">Background</span>
                    <div className="space-y-3">
                      <div>
                        <label className="text-white/80 text-xs">Upload</label>
                        <input
                          type="file"
                          accept="image/*"
                          onChange={handleImageUpload}
                          className="hidden"
                          id="background-upload"
                        />
                        <label
                          htmlFor="background-upload"
                          className="text-neon w-full px-3 py-2 bg-white/10 border border-white/20 rounded-lg  text-sm cursor-pointer hover:bg-white/20 transition-all flex items-center justify-center"
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
                          Remove
                        </button>
                      )}
                    </div>
                  </div>
                )}
                {showWidgetSettings && (
                  <div className="bg-black/80 backdrop-blur-xl rounded-2xl p-4 shadow-2xl border border-white/20 z-100">
                    <span className="text-neon text-sm font-semibold mb-3 prose">Widgets</span>
                    <div className="space-y-2 max-h-64 overflow-y-auto">
                      {homeApps.filter(app => app.widget).map(app => (
                        <div key={app.id} className="flex gap-2 items-center justify-between text-white/80 text-sm p-2 rounded-lg hover:bg-white/10 transition-colors">
                          <span>{app.label}</span>
                          <button
                            onClick={() => toggleWidget(app.id)}
                            className={`thin !text-neon rounded-full text-xs font-medium transition-all ${widgetSettings[app.id]?.hide
                              ? '!bg-white/10 hover:!bg-white/20'
                              : '!bg-gray-600/50 hover:!bg-gray-600/70'
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
                )}
              </div>
            </div>
          )}

          {!isEditMode && <>
            {/* <a
              href="mailto:support@hyperware.ai"
              className="button bg-gradient-to-r from-gray-600 to-gray-700 !text-neon text-sm ml-auto !p-2"
              target="_blank"
              rel="noopener noreferrer"
              title="Get help & support"
            >
              <BsEnvelope />
            </a> */}
            <button
              onClick={() => setEditMode(true)}
              className="bg-gradient-to-r from-gray-600 to-gray-700 !text-neon text-sm !px-2 ml-auto"
              title="Edit apps, widgets, and background"
            >
              <BsPencilSquare />
            </button>
            <div className="relative">
              <button
                onClick={handleNotificationClick}
                className="bg-gradient-to-r from-gray-600 to-gray-700 !text-neon text-sm !p-2 relative"
                title="Notifications"
                data-notification-button
              >
                <BsBell className="w-4 h-4" />
                {showNotificationIndicator && (
                  <span className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full animate-pulse" />
                )}
              </button>
              <NotificationMenu />
            </div>
            <button
              className=" thin  grow self-stretch max-w-sm md:max-w-md !justify-start !bg-black/10 dark:!bg-white/10 backdrop-blur-xl"
              onClick={() => toggleAppDrawer()}
            >
              <BsSearch
                className="opacity-50 dark:placeholder-white dark:text-white"
              />
              <span>
                Search apps...
              </span>
            </button>
          </>}

        </div>

        {showOnboarding && (
          <Modal
            onClose={() => setShowOnboarding(false)}
            title="Welcome to Hyperware"
          >
            <p>Your gateway to the internet, reimagined.</p>
            <p>Your node, your device: customize the interface, pin your favorite apps.</p>
            <p>Your node, your data: take full control over your information.</p>
            <div className="flex flex-col items-center gap-2 ">
              <button
                onClick={() => {
                  setDoNotShowOnboardingAgain(true)
                  setShowOnboarding(false)
                }}
                className="clear text-sm self-stretch !rounded-full"
              >
                Don't show again
              </button>
              <button
                onClick={() => setShowOnboarding(false)}
                className="bg-neon text-black font-bold self-stretch md:grow !rounded-full"
              >
                Let's go
              </button>
            </div>
          </Modal>
        )}


        {/* <div className="hidden md:block absolute bottom-24 left-1/2 -translate-x-1/2 text-black/30 dark:text-white/30 text-xs bg-white/50 dark:bg-black/50 backdrop-blur rounded-lg px-3 py-2">
          <div className="flex items-center gap-4">
            <span><kbd className="p-1 bg-black/10 dark:bg-white/10 rounded text-xs">A</kbd> All apps</span>
            <span><kbd className="p-1 bg-black/10 dark:bg-white/10 rounded text-xs">S</kbd> Recent apps</span>
            <span><kbd className="p-1 bg-black/10 dark:bg-white/10 rounded text-xs">H</kbd> Home</span>
            <span><kbd className="p-1 bg-black/10 dark:bg-white/10 rounded text-xs">1-9</kbd> Switch apps</span>
          </div>
        </div> */}
      </div>
    </div>
  );
};
