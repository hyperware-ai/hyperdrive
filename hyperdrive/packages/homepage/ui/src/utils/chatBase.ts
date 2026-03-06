const DEFAULT_CHAT_BASE = '/chat:homepage:sys';

export const getChatBasePath = (): string => {
  const envBase = import.meta.env.VITE_CHAT_BASE;
  const base = envBase && envBase !== '/' ? envBase : DEFAULT_CHAT_BASE;
  return base.endsWith('/') ? base.slice(0, -1) : base;
};

export const buildChatPath = (path: string): string => {
  const base = getChatBasePath();
  if (!path) return base;
  if (path.startsWith('/')) {
    return `${base}${path}`;
  }
  return `${base}/${path}`;
};
