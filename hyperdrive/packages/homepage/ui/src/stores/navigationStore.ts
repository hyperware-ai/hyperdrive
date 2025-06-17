import { create } from 'zustand';
import type { HomepageApp, RunningApp } from '../types/app.types';

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

export const useNavigationStore = create<NavigationStore>((set, get) => ({
  runningApps: [],
  currentAppId: null,
  isAppDrawerOpen: false,
  isRecentAppsOpen: false,

  openApp: (app) => {
    console.log('openApp called with:', app);

    // Don't open apps without a valid path
    if (!app.path && !app.process && !app.publisher) {
      console.warn(`Cannot open app ${app.label}: No valid path`);
      return;
    }

    // Check if we need to open in a new tab (localhost with subdomain)
    const currentHost = window.location.host;
    const isLocalhost = currentHost.includes("localhost");

    if (isLocalhost && app.process && app.publisher) {
      const generateSubdomain = (process: string, publisher: string) => {
        return `${process}-${publisher}`.toLowerCase()
          .split('')
          .map(c => c.match(/[a-zA-Z0-9]/) ? c : '-')
          .join('');
      };

      const expectedSubdomain = generateSubdomain(app.package_name, app.publisher);
      const needsSubdomain = !currentHost.startsWith(expectedSubdomain);

      if (needsSubdomain) {
        // Open in new tab for localhost
        const appUrl = app.path || `/app:${app.process}:${app.publisher}.os/`;
        const protocol = window.location.protocol;
        const port = window.location.port ? `:${window.location.port}` : '';

        // Fix: Extract just the hostname without port for localhost
        const hostname = currentHost.split(':')[0]; // 'localhost' from 'localhost:3000'
        const baseDomain = hostname; // For localhost, we just use 'localhost'

        const subdomainUrl = `${protocol}//${expectedSubdomain}.${baseDomain}${port}${appUrl}`;

        // Debug logging
        console.log('Navigation Debug:', {
          app: app.label,
          currentHost,
          hostname,
          baseDomain,
          expectedSubdomain,
          needsSubdomain,
          subdomainUrl,
          protocol,
          port
        });

        const newWindow = window.open(subdomainUrl, '_blank');
        console.log('window.open result:', newWindow);

        if (!newWindow) {
          console.error('Failed to open new window - possibly blocked by popup blocker');
        }

        // Close overlays to return to homepage
        set({ isAppDrawerOpen: false, isRecentAppsOpen: false });
        return;
      }
    }

    // Normal iframe behavior for non-localhost or same-subdomain apps
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
