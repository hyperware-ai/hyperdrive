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