/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CHAT_BASE?: string;
  readonly VITE_APP_VERSION?: string;
  readonly VITE_NODE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
