import { Chat } from '#caller-utils';

const normalize = (value?: string | null): string => (value ?? '').trim();

export const getChatDisplayName = (chat: Chat.Chat): string => {
  const nickname = normalize(chat.counterparty_profile?.name);
  return nickname || chat.counterparty;
};

export const getChatNodeSubtitle = (chat: Chat.Chat): string => chat.counterparty;

export const getCounterpartyBaseAddress = (chat: Chat.Chat): string | null => {
  const baseAddress = normalize(chat.counterparty_profile?.base_address);
  return baseAddress || null;
};
