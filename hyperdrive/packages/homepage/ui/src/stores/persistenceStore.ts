import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { Position, Size } from '../types/app.types';

interface PersistenceStore {
  homeScreenApps: string[];
  appPositions: { [key: string]: Position };
  widgetSettings: { [key: string]: { hide?: boolean; position?: Position; size?: Size } };
  backgroundImage: string | null;

  addToHomeScreen: (appId: string) => void;
  removeFromHomeScreen: (appId: string) => void;
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

      setBackgroundImage: (imageUrl) => {
        set({ backgroundImage: imageUrl });
      },
    }),
    {
      name: 'android-homescreen-store',
      storage: createJSONStorage(() => localStorage),
    }
  )
);