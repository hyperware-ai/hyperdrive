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