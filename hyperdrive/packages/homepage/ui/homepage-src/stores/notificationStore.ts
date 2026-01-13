import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface AppNotification {
  id: string;
  appId: string;
  appLabel: string;
  title: string;
  body: string;
  icon?: string;
  timestamp: number;
  read: boolean;
  seen: boolean;
}

interface NotificationStore {
  notifications: AppNotification[];
  permissionGranted: boolean;
  menuOpen: boolean;
  hasVapidKey: boolean | null;

  // Actions
  addNotification: (notification: Omit<AppNotification, 'id' | 'timestamp' | 'read' | 'seen'>) => void;
  markAsRead: (id: string) => void;
  markAllAsRead: () => void;
  markAsSeen: (id: string) => void;
  markAllAsSeen: () => void;
  clearNotifications: () => void;
  setPermissionGranted: (granted: boolean) => void;
  setMenuOpen: (open: boolean) => void;
  setHasVapidKey: (hasKey: boolean | null) => void;

  // Computed
  getUnreadCount: () => number;
  getUnseenCount: () => number;
  getUnreadNotifications: () => AppNotification[];
  getUnseenNotifications: () => AppNotification[];
}

export const useNotificationStore = create<NotificationStore>()(
  persist(
    (set, get) => ({
      notifications: [],
      permissionGranted: false,
      menuOpen: false,
      hasVapidKey: null,

      addNotification: (notification) => {
        const newNotification: AppNotification = {
          ...notification,
          id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
          timestamp: Date.now(),
          read: false,
          seen: false,
        };

        set((state) => ({
          notifications: [newNotification, ...state.notifications].slice(0, 100), // Keep max 100 notifications
        }));
      },

      markAsRead: (id) => {
        set((state) => ({
          notifications: state.notifications.map((n) =>
            n.id === id ? { ...n, read: true } : n
          ),
        }));
      },

      markAllAsRead: () => {
        set((state) => ({
          notifications: state.notifications.map((n) => ({ ...n, read: true })),
        }));
      },

      markAsSeen: (id) => {
        set((state) => ({
          notifications: state.notifications.map((n) =>
            n.id === id ? { ...n, seen: true } : n
          ),
        }));
      },

      markAllAsSeen: () => {
        set((state) => ({
          notifications: state.notifications.map((n) => ({ ...n, seen: true })),
        }));
      },

      clearNotifications: () => {
        set({ notifications: [] });
      },

      setPermissionGranted: (granted) => {
        set({ permissionGranted: granted });
      },

      setMenuOpen: (open) => {
        set({ menuOpen: open });
        // When closing menu, mark all as seen
        if (!open) {
          get().markAllAsSeen();
        }
      },

      setHasVapidKey: (hasKey) => {
        set({ hasVapidKey: hasKey });
      },

      getUnreadCount: () => {
        return get().notifications.filter((n) => !n.read).length;
      },

      getUnseenCount: () => {
        return get().notifications.filter((n) => !n.seen).length;
      },

      getUnreadNotifications: () => {
        return get().notifications.filter((n) => !n.read);
      },

      getUnseenNotifications: () => {
        return get().notifications.filter((n) => !n.seen);
      },
    }),
    {
      name: 'notification-storage',
      partialize: (state) => ({
        notifications: state.notifications,
        permissionGranted: state.permissionGranted,
      }),
    }
  )
);
