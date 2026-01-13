import { useCallback, useEffect, useState } from 'react';
import './App.css';
import './styles/button-selectable.css';
import './styles/bottom-bar.css';
import './homepage/styles/animations.css';
import { useChatStore } from './store/chat';
import SplashScreen from './components/SplashScreen/SplashScreen';
import ChatView from './components/Chat/ChatView';
import { useGroupStore } from './store/groups';
import GroupView from './components/Groups/GroupView';
import GroupJoinModal from './components/Groups/GroupJoinModal';
import { parseGroupJoinLink } from './utils/groupLinks';
import type { GroupJoinTarget } from './utils/groupLinks';
import { useAppStore } from './homepage/stores/appStore';
import { useNavigationStore } from './homepage/stores/navigationStore';
import type { HomepageApp } from './homepage/types/app.types';
import { AppContainer } from './homepage/components/AppContainer';
import { AppDrawer } from './homepage/components/AppDrawer';
import { RecentApps } from './homepage/components/RecentApps';
import { OmniButton } from './homepage/components/OmniButton';
import UpdateNotification from './homepage/components/UpdateNotification';
import InstallPrompt from './homepage/components/InstallPrompt';
import { IframeMessageType, isIframeMessage } from './homepage/types/messages';
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';

dayjs.extend(relativeTime);

type MainTab = 'chat' | 'apps' | 'wallet';

function App() {
  const {
    nodeId,
    isConnected,
    activeChat,
    error,
    initialize,
    clearError,
    chats,
    isLoading,
  } = useChatStore();
  const { activeGroup, loadGroups, fetchReplicationState } = useGroupStore();
  const [pendingJoin, setPendingJoin] = useState<GroupJoinTarget | null>(null);
  const [activeTab, setActiveTab] = useState<MainTab>('chat');

  const { apps, setApps } = useAppStore();
  const {
    runningApps,
    currentAppId,
    isAppDrawerOpen,
    isRecentAppsOpen,
    initBrowserBackHandling,
    openApp,
  } = useNavigationStore();

  const fetchApps = useCallback(async () => {
    try {
      const res = await fetch('/apps', { credentials: 'include' });
      const fetchedApps = (await res.json()) as HomepageApp[];
      setApps(fetchedApps);
      return fetchedApps;
    } catch (error) {
      console.warn('Failed to fetch apps from backend:', error);
      const fallbackApps: HomepageApp[] = [
        {
          id: '1',
          process: 'settings',
          package_name: 'settings',
          publisher: 'sys',
          path: '/app:settings:sys.os/',
          label: 'Settings',
          order: 1,
          favorite: true,
        },
        {
          id: '2',
          process: 'files',
          package_name: 'files',
          publisher: 'sys',
          path: '/app:files:sys.os/',
          label: 'Files',
          order: 2,
          favorite: false,
        },
        {
          id: '3',
          process: 'terminal',
          package_name: 'terminal',
          publisher: 'sys',
          path: '/app:terminal:sys.os/',
          label: 'Terminal',
          order: 3,
          favorite: false,
        },
        {
          id: '4',
          process: 'browser',
          package_name: 'browser',
          publisher: 'sys',
          path: '/app:browser:sys.os/',
          label: 'Browser',
          order: 4,
          favorite: true,
        },
        {
          id: '5',
          process: 'app-store',
          package_name: 'app-store',
          publisher: 'sys',
          path: '/main:app-store:sys/',
          label: 'App Store',
          order: 5,
          favorite: false,
          widget: 'true',
        },
      ];
      setApps(fallbackApps);
      return fallbackApps;
    }
  }, [setApps]);

  // Initialize chat on mount
  useEffect(() => {
    initialize();
  }, [initialize]);

  // Prime group data when we have a connection
  useEffect(() => {
    if (isConnected) {
      loadGroups();
      fetchReplicationState(null);
    }
  }, [isConnected, loadGroups, fetchReplicationState]);

  // Initialize browser back handling and app list
  useEffect(() => {
    initBrowserBackHandling();
    fetchApps();
  }, [initBrowserBackHandling, fetchApps]);

  // Open app from hash if we refreshed
  useEffect(() => {
    if (window?.location?.hash?.startsWith('#app-')) {
      const hashWithoutPrefix = window.location.hash.replace('#app-', '');
      const appNameMatch = hashWithoutPrefix.match(/^([^/?]+)/);
      const appNameToOpen = appNameMatch ? appNameMatch[1] : '';
      const remainder = hashWithoutPrefix.slice(appNameToOpen.length);
      const appToOpen = apps?.find((app) => app?.id === appNameToOpen);
      if (appToOpen) {
        openApp(appToOpen, remainder || undefined);
      }
    }
  }, [apps, openApp]);

  useEffect(() => {
    const handleMessage = async (event: MessageEvent) => {
      if (!isIframeMessage(event.data)) {
        return;
      }

      let allGood = true;

      const isValidOrigin = (() => {
        const currentOrigin = window.location.origin;
        const eventOrigin = event.origin;

        if (eventOrigin === currentOrigin) {
          return true;
        }

        const appStoreOrigin = currentOrigin.replace(/^(https?:\/\/)/, '$1app-store-sys.');
        if (eventOrigin === appStoreOrigin) {
          return true;
        }

        const currentUrl = new URL(currentOrigin);
        const eventUrl = new URL(eventOrigin);

        if (currentUrl.protocol !== eventUrl.protocol || currentUrl.port !== eventUrl.port) {
          return false;
        }

        if (currentUrl.hostname.includes('localhost')) {
          return eventUrl.hostname.endsWith('.localhost') || eventUrl.hostname === 'localhost';
        }

        const getCurrentBaseDomain = (hostname: string) => {
          const parts = hostname.split('.');
          return parts.length >= 2 ? parts.slice(-2).join('.') : hostname;
        };

        const currentBaseDomain = getCurrentBaseDomain(currentUrl.hostname);
        const eventBaseDomain = getCurrentBaseDomain(eventUrl.hostname);

        return currentBaseDomain === eventBaseDomain;
      })();

      if (!isValidOrigin) {
        allGood = false;
      }

      if (!isIframeMessage(event.data)) {
        allGood = false;
      }

      if (!allGood) {
        return;
      }

      if (event.data.type === IframeMessageType.OPEN_APP) {
        const { id } = event.data;
        const fetchedApps = await fetchApps();
        const appMatches = fetchedApps?.filter((app) => app.id.endsWith(':' + id));
        if (appMatches?.length > 1) {
          console.error('Multiple apps found with the same id:', { id, fetchedApps });
        } else if (appMatches.length === 0) {
          console.error('App not found:', { id, fetchedApps });
        }
        const app = appMatches?.[0];
        if (app) {
          openApp(app);
        }
      } else if (event.data.type === IframeMessageType.APP_LINK_CLICKED) {
        const { url } = event.data;
        const app = apps.find((entry) => entry.id.endsWith('app-store:sys'));
        if (app) {
          openApp(app, url);
        }
      } else if (event.data.type === IframeMessageType.HW_LINK_CLICKED) {
        const { url } = event.data;
        const urlParts = url
          .split('/')
          .filter((part) => part !== '' && part !== null && part !== undefined);
        const appName = urlParts[0];
        const path = urlParts.slice(1).join('/');
        const app = apps.find((entry) => entry.id.endsWith(appName));
        if (app) {
          openApp(app, path || undefined);
        }
      }
    };
    window.addEventListener('message', handleMessage);

    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, [apps, fetchApps, openApp]);

  useEffect(() => {
    const parsed = parseGroupJoinLink(window.location.pathname);
    if (parsed) {
      setPendingJoin(parsed);
      const parts = window.location.pathname.split('/').filter(Boolean);
      const joinIndex = parts.indexOf('join-group');
      if (joinIndex !== -1) {
        const baseParts = parts.slice(0, joinIndex);
        const basePath = baseParts.length ? `/${baseParts.join('/')}/` : '/';
        window.history.replaceState(
          {},
          '',
          `${basePath}${window.location.search}${window.location.hash}`,
        );
      }
    }
  }, []);

  useEffect(() => {
    const handleLinkClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      const anchor = target?.closest('a');
      const href = anchor?.getAttribute('href');
      if (!href) return;
      const parsed = parseGroupJoinLink(href);
      if (!parsed) return;
      event.preventDefault();
      event.stopPropagation();
      setPendingJoin(parsed);
    };
    document.addEventListener('click', handleLinkClick, true);
    return () => document.removeEventListener('click', handleLinkClick, true);
  }, []);

  const isChatDetail = activeTab === 'chat' && Boolean(activeChat || activeGroup);
  const isBottomBarHidden = isChatDetail || Boolean(currentAppId);
  const shouldShowOmniButton = runningApps.length > 0;
  const showAppsView = activeTab === 'apps' && !currentAppId;

  if (chats.length === 0 && !nodeId && !error && isLoading) {
    return (
      <div className="app-loading">
        <div className="spinner" />
        <p>Connecting to Hyperware...</p>
      </div>
    );
  }

  if (!isConnected && error && chats.length === 0) {
    return (
      <div className="app-error">
        <h2>Connection Error</h2>
        <p>{error}</p>
        <button onClick={() => window.location.reload()}>
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="app app-shell">
      {error && (
        <div className="error-banner">
          {error}
          <button onClick={clearError} className="dismiss-button">
            ×
          </button>
        </div>
      )}

      <div className={`app-main ${isBottomBarHidden ? 'app-main--bar-hidden' : ''}`}>
        {showAppsView ? (
          <AppDrawer forceOpen disableBackdropClose zIndexClass="z-30" />
        ) : activeGroup ? (
          <GroupView />
        ) : activeChat ? (
          <ChatView />
        ) : (
          <SplashScreen />
        )}
      </div>

      {runningApps.map((app) => (
        <AppContainer
          key={app.id}
          app={app}
          isVisible={currentAppId === app.id && !isAppDrawerOpen && !isRecentAppsOpen}
        />
      ))}

      <RecentApps />

      {shouldShowOmniButton && <OmniButton />}

      <UpdateNotification />

      <InstallPrompt />

      <nav className={`bottom-bar ${isBottomBarHidden ? 'bottom-bar--hidden' : ''}`}>
        <button
          className={`bottom-bar__item ${activeTab === 'chat' ? 'bottom-bar__item--active' : ''}`}
          onClick={() => setActiveTab('chat')}
          type="button"
        >
          Chat
        </button>
        <button
          className={`bottom-bar__item ${activeTab === 'apps' ? 'bottom-bar__item--active' : ''}`}
          onClick={() => setActiveTab('apps')}
          type="button"
        >
          Apps
        </button>
        <button
          className="bottom-bar__item bottom-bar__item--disabled"
          type="button"
          aria-disabled="true"
          disabled
        >
          Wallet (coming soon)
        </button>
      </nav>

      {pendingJoin && (
        <GroupJoinModal
          host={pendingJoin.host}
          keyValue={pendingJoin.key}
          onClose={() => setPendingJoin(null)}
        />
      )}
    </div>
  );
}

export default App;
