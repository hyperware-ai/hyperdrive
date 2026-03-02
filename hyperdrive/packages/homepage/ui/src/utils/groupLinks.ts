export type GroupJoinTarget = {
  host: string;
  key: string;
};

export function parseGroupJoinLink(input: string): GroupJoinTarget | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  const cleaned = trimmed.replace(/[)>.,]+$/, '');
  const isHw = cleaned.startsWith('hw://');
  const isPath = cleaned.startsWith('/');
  if (!isHw && !isPath) return null;
  const normalized = isHw ? cleaned.slice('hw://'.length) : cleaned;
  const withoutQuery = normalized.split('?')[0]?.split('#')[0] ?? normalized;
  const parts = withoutQuery.split('/').filter(Boolean);
  const joinIndex = parts.indexOf('join-group');
  if (joinIndex === -1) return null;

  const host = parts[joinIndex + 1];
  const key = parts[joinIndex + 2];
  if (!host || !key) return null;

  return { host, key };
}
