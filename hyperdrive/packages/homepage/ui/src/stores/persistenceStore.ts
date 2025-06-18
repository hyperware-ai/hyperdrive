import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { Position, Size } from '../types/app.types';

interface PersistenceStore {
  homeScreenApps: string[];
  dockApps: string[];
  appPositions: { [key: string]: Position };
  widgetSettings: { [key: string]: { hide?: boolean; position?: Position; size?: Size } };
  backgroundImage: string | null;

  addToHomeScreen: (appId: string) => void;
  removeFromHomeScreen: (appId: string) => void;
  addToDock: (appId: string, index?: number) => void;
  removeFromDock: (appId: string) => void;
  moveItem: (appId: string, position: Position) => void;
  toggleWidget: (appId: string) => void;
  setWidgetPosition: (appId: string, position: Position) => void;
  setWidgetSize: (appId: string, size: Size) => void;
  setBackgroundImage: (imageUrl: string | null) => void;
}

export const usePersistenceStore = create<PersistenceStore>()(
  persist(
    (set) => ({
      homeScreenApps: [],
      dockApps: [],
      appPositions: {},
      widgetSettings: {},
      backgroundImage: null,

      addToHomeScreen: (appId) => {
        set((state) => ({
          homeScreenApps: [...state.homeScreenApps, appId],
          // Default position for apps at bottom of screen
          appPositions: {
            ...state.appPositions,
            [appId]: {
              x: Math.min(Math.random() * (window.innerWidth - 100), window.innerWidth - 80),
              y: Math.min(window.innerHeight - 200 - Math.random() * 100, window.innerHeight - 80)
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
                x: Math.min(Math.random() * (window.innerWidth - 300), window.innerWidth - 300),
                y: Math.min(50 + Math.random() * 100, window.innerHeight - 200)
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

      setBackgroundImage: (imageUrl) => {
        set({ backgroundImage: imageUrl });
      },

      addToDock: (appId, index) => {
        set((state) => {
          const newDockApps = [...state.dockApps];
          // Remove from dock if already there
          const existingIndex = newDockApps.indexOf(appId);
          if (existingIndex !== -1) {
            newDockApps.splice(existingIndex, 1);
          }
          // Add at specific index or at end
          if (index !== undefined && index >= 0 && index <= newDockApps.length) {
            newDockApps.splice(index, 0, appId);
          } else {
            newDockApps.push(appId);
          }
          // Limit dock to 4 apps
          if (newDockApps.length > 4) {
            newDockApps.length = 4;
          }
          return { dockApps: newDockApps };
        });
      },

      removeFromDock: (appId) => {
        set((state) => ({
          dockApps: state.dockApps.filter(id => id !== appId)
        }));
      },
    }),
    {
      name: 'android-homescreen-store',
      storage: createJSONStorage(() => localStorage),
    }
  )
);