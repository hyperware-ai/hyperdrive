import { create } from 'zustand';
import type { HomepageApp, RunningApp } from '../types/app.types';

interface NavigationStore {
  runningApps: RunningApp[];
  currentAppId: string | null;
  isAppDrawerOpen: boolean;
  isRecentAppsOpen: boolean;

  openApp: (app: HomepageApp, query?: string) => void;
  closeApp: (appId: string) => void;
  switchToApp: (appId: string) => void;
  toggleAppDrawer: () => void;
  toggleRecentApps: () => void;
  closeAllOverlays: () => void;
  initBrowserBackHandling: () => void;
  handleBrowserBack: (state: any) => void;
}

export const useNavigationStore = create<NavigationStore>((set, get) => ({
  runningApps: [],
  currentAppId: null,
  isAppDrawerOpen: false,
  isRecentAppsOpen: false,

  // Initialize browser back button handling
  initBrowserBackHandling: () => {
    // Only add listener once
    if (typeof window !== 'undefined' && !window.hasBackHandler) {
      const handlePopState = (event: PopStateEvent) => {
        get().handleBrowserBack(event.state);
      };

      window.addEventListener('popstate', handlePopState);
      window.hasBackHandler = true;

      // Set initial state
      window.history.replaceState({ type: 'homepage' }, '', window.location.href);
    }
  },

  // Handle browser back button presses
  handleBrowserBack: (state) => {
    const { runningApps, currentAppId, isAppDrawerOpen, isRecentAppsOpen } = get();

    // Close overlays first
    if (isAppDrawerOpen || isRecentAppsOpen) {
      set({ isAppDrawerOpen: false, isRecentAppsOpen: false });
      return;
    }

    // If we have a current app, go back to previous app or homepage
    if (currentAppId && state?.type === 'app' && state?.appId !== currentAppId) {
      const targetApp = runningApps.find(app => app.id === state.appId);
      if (targetApp) {
        set({ currentAppId: state.appId });
      } else {
        // App no longer running, go to homepage
        set({ currentAppId: null });
      }
    } else if (currentAppId && state?.type === 'homepage') {
      // Go back to homepage
      set({ currentAppId: null });
    }
    // If already on homepage, let default browser behavior handle it
  },

  openApp: async (app: HomepageApp, query?: string) => {
    console.log('openApp called with:', { app, query });

    // Don't open apps without a valid path
    if (!app.path || !app.process || !app.publisher) {
      const e = `Cannot open app ${app.label}: No valid path`
      console.warn(e);
      alert(e);
      return;
    }

    // Check if we need to open in a new tab (localhost)
    const isLocalhost = window.location.host.includes("localhost");
    if (isLocalhost) {
      console.log('[homepage] opening app in new tab:', { app });
      // don't use secure subdomain for localhost
      const path = app.path.replace(/^(https?:\/\/)(.*)localhost/, '$1localhost') + (query || '');
      console.log({ path })
      window.open(path, '_blank');
      set({ isAppDrawerOpen: false, isRecentAppsOpen: false });
      return;
    }
    console.log('[homepage] opening app in iframe:', { app });

    // Normal iframe behavior for non-localhost or same-subdomain apps
    const { runningApps, currentAppId } = get();
    const existingApp = runningApps.find(a => a.id === app.id);

    if (existingApp && currentAppId === app.id) {
      console.log('[homepage] app already open:', { app });
      return;
    }

    let maybeSlash = '';

    if (query && query[0] && query[0] !== '?' && query[0] !== '/') {
        // autoprepend a slash for the window history when the query type is unknown
        console.log('autoprepended / to unknown query format');
        maybeSlash = '/'
    }

    // Add to browser history for back button support
    window?.history?.pushState(
      { type: 'app', appId: app.id, previousAppId: currentAppId },
      '',
      `#app-${app.id}${maybeSlash}${query || ''}`
    );

    if (existingApp) {
      set({
        runningApps: runningApps.map(rApp => {
            const path  = `${app.path}${query || ''}`;
            console.log(path, rApp.id, app.id);
            if (rApp.id === app.id) {
                console.log('found rApp')
                return {
                    ...rApp,
                    path
                }
            }
            return rApp;
        }),
        currentAppId: app.id,
        isAppDrawerOpen: false,
        isRecentAppsOpen: false
      });
    } else {
      set({
        runningApps: [...runningApps, {
          ...app,
          path: `${app.path}${query || ''}`,
          openedAt: Date.now()
        }],
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

    // Update browser history when closing current app
    if (currentAppId === appId && typeof window !== 'undefined') {
      if (newCurrentApp) {
        window.history.pushState(
          { type: 'app', appId: newCurrentApp },
          '',
          `#app-${newCurrentApp}`
        );
      } else {
        window.history.pushState({ type: 'homepage' }, '', '#');
      }
    }

    set({
      runningApps: newRunningApps,
      currentAppId: newCurrentApp,
    });
  },

  switchToApp: (appId) => {
    // Add to browser history when switching apps
    if (typeof window !== 'undefined') {
      window.history.pushState(
        { type: 'app', appId, previousAppId: get().currentAppId },
        '',
        `#app-${appId}`
      );
    }

    set({ currentAppId: appId, isRecentAppsOpen: false });
  },

  toggleAppDrawer: () => set((state) => ({ isAppDrawerOpen: !state.isAppDrawerOpen, isRecentAppsOpen: false })),
  toggleRecentApps: () => set((state) => ({ isRecentAppsOpen: !state.isRecentAppsOpen, isAppDrawerOpen: false })),
  closeAllOverlays: () => {
    // Add to browser history when returning to homepage
    if (typeof window !== 'undefined' && get().currentAppId) {
      window.history.pushState({ type: 'homepage' }, '', '#');
    }

    set({ isAppDrawerOpen: false, isRecentAppsOpen: false, currentAppId: null });
  },
}));

// Global type extension
declare global {
  interface Window {
    hasBackHandler?: boolean;
  }
}
