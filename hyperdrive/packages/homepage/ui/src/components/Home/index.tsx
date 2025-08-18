import { useState, useEffect } from 'react';
import { useAppStore } from '../../stores/appStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { HomeScreen } from './components/HomeScreen';
import { AppContainer } from './components/AppContainer';
import { AppDrawer } from './components/AppDrawer';
import { RecentApps } from './components/RecentApps';
import { OmniButton } from './components/OmniButton';
import UpdateNotification from '../UpdateNotification';
import InstallPrompt from '../InstallPrompt';
import './styles/animations.css';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
dayjs.extend(relativeTime);

export default function Home() {
  const { apps, setApps } = useAppStore();
  const {
    runningApps,
    currentAppId,
    isAppDrawerOpen,
    isRecentAppsOpen,
    toggleRecentApps,
    switchToApp,
    toggleAppDrawer,
    closeAllOverlays,
    initBrowserBackHandling,
    openApp,
  } = useNavigationStore();
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      if (!event?.data?.type) {
        // ignore other iframe messages e.g. metamask 
        return;
      }
      let allGood = true;
      // App Store is a good boy, he can send messages to us
      const replaced = window.location.origin.replace(/^(https?:\/\/)/, '$1app-store-sys.')
      if (
        replaced !== event.origin
      ) {
        console.log('expected same origin or app-store-sys, got:', event.origin, window.location.origin, replaced);
        allGood = false;
      }
      if (event.data.type !== 'OPEN_APP') {
        console.log('expected OPEN_APP, got:', event.data.type);
        allGood = false;
      }
      if (!allGood) {
        console.log('not all good', { event });
        return;
      }

      if (event.data.type === 'OPEN_APP') {
        const { id } = event.data;
        const appMatches = apps.filter(app => app.id.endsWith(':' + id));
        if (appMatches.length > 1) {
          console.error('Multiple apps found with the same id:', { id, apps });
        } else if (appMatches.length === 0) {
          console.error('App not found:', { id, apps });
        }
        const app = appMatches[0];
        if (app) {
          openApp(app);
        } else {
          console.error('App not found:', { id, apps });
        }
      }
    };
    window.addEventListener('message', handleMessage);

    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, [apps, openApp]);


  // Keyboard shortcuts for desktop
  useEffect(() => {
    const handleKeyPress = (e: KeyboardEvent) => {
      // Ignore if user is typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      // Single key shortcuts
      switch (e.key.toLowerCase()) {
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

  // Fetch apps from backend and initialize browser back handling
  useEffect(() => {
    // Initialize browser back button handling
    initBrowserBackHandling();

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
  }, [setApps, initBrowserBackHandling]);

  if (loading) {
    return (
      <div className="fixed inset-0 bg-gradient-to-br from-gray-900 to-black flex items-center justify-center">
        <div className="text-center flex flex-col items-center gap-4">
          <div className="w-16 h-16 border-4 border-gray-700 border-t-blue-500 rounded-full animate-spin "></div>
          <div className="text-gray-300 text-xl">Loading Hyperware...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 overflow-hidden" style={{ touchAction: 'none' }}>

      <HomeScreen />


      {runningApps.map(app => (
        <AppContainer
          key={app.id}
          app={app}
          isVisible={currentAppId === app.id && !isAppDrawerOpen && !isRecentAppsOpen}
        />
      ))}


      <AppDrawer />
      <RecentApps />


      <OmniButton />


      <UpdateNotification />


      <InstallPrompt />
    </div>
  );
}
