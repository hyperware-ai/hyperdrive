export interface HomepageApp {
  id: string;
  process: string;
  package_name: string;
  publisher: string;
  path?: string;
  label: string;
  base64_icon?: string;
  widget?: string;
  order: number;
  favorite: boolean;
}

export interface RunningApp extends HomepageApp {
  openedAt: number;
}

export interface Position {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

// WebSocket message types (mirrors Rust WsMessage enum)
export type WsMessage = WsAppsUpdate;

export interface WsAppsUpdate {
  kind: 'apps_update';
  data: HomepageApp[];
}

// Type guard for WsMessage
export function isWsMessage(obj: unknown): obj is WsMessage {
  if (typeof obj !== 'object' || obj === null) return false;
  const msg = obj as Record<string, unknown>;
  return msg.kind === 'apps_update' && Array.isArray(msg.data);
}