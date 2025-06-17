import { useState, useEffect } from 'react';
import { useAppStore } from '../../stores/appStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { HomeScreen } from './components/HomeScreen';
import { AppContainer } from './components/AppContainer';
import { AppDrawer } from './components/AppDrawer';
import { RecentApps } from './components/RecentApps';
import { GestureZone } from './components/GestureZone';
import './styles/animations.css';

export default function AndroidHomescreen() {
  const { setApps } = useAppStore();
  const { 
    runningApps, 
    currentAppId, 
    isAppDrawerOpen, 
    isRecentAppsOpen, 
    toggleRecentApps, 
    switchToApp, 
    toggleAppDrawer, 
    closeAllOverlays 
  } = useNavigationStore();
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