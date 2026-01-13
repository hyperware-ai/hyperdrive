import { create } from 'zustand';
import type { HomepageApp } from '../types/app.types';
import { isWsMessage } from '../types/app.types';

// Homepage is special - it's served at root, not at a subdomain
const WS_URL = `${window.location.protocol.replace('http', 'ws')}//${window.location.host}/`;

interface AppStore {
  apps: HomepageApp[];
  setApps: (apps: HomepageApp[]) => void;
  isEditMode: boolean;
  setEditMode: (mode: boolean) => void;
  ws: WebSocket | null;
}

// Create WebSocket connection
function createWebSocket(setApps: (apps: HomepageApp[]) => void): WebSocket {
  const ws = new WebSocket(WS_URL);

  ws.onmessage = (event: MessageEvent<string>) => {
    try {
      const parsed: unknown = JSON.parse(event.data);
      if (isWsMessage(parsed) && parsed.kind === 'apps_update') {
        setApps(parsed.data);
      }
    } catch (e) {
      console.error('[Homepage WS] Error parsing message:', e);
    }
  };

  ws.onerror = (event: Event) => {
    console.error('[Homepage WS] Connection error:', event);
  };

  return ws;
}

export const useAppStore = create<AppStore>((set) => {
  // Initialize WebSocket after store is created
  const ws = createWebSocket((apps) => set({ apps }));

  return {
    apps: [],
    setApps: (apps) => set({ apps }),
    isEditMode: false,
    setEditMode: (isEditMode) => set({ isEditMode }),
    ws,
  };
});
