export const GROUP_PERMISSION_FLAGS = {
  SEND_MESSAGES: 1 << 0,
  CREATE_THREADS: 1 << 1,
  INVITE_MEMBERS: 1 << 2,
  MANAGE_ROLES: 1 << 3,
  MANAGE_SETTINGS: 1 << 4,
} as const;

export type GroupPermissionKey = keyof typeof GROUP_PERMISSION_FLAGS;

export function hasGroupPermission(
  permissions: number | null | undefined,
  flag: GroupPermissionKey,
): boolean {
  if (permissions === null || permissions === undefined) {
    return false;
  }
  const bit = GROUP_PERMISSION_FLAGS[flag];
  return (permissions & bit) === bit;
}
