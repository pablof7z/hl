import { browser } from '$app/environment';

/**
 * When an invitee clicks a /r/<slug>/join/<code> URL without being signed in,
 * we park the (groupId, code) pair in sessionStorage before sending them
 * through onboarding. After the session establishes, the onboarding flow (or
 * any logged-in entry point) reads this back and redirects the user straight
 * to the join page to complete acceptance.
 *
 * sessionStorage (not localStorage) scopes this to the tab and clears on
 * close, which matches the invite-is-pending-right-now semantics.
 */

const KEY = 'hl:pending-invite';

export type PendingInvite = { groupId: string; code: string };

export function setPendingInvite(invite: PendingInvite): void {
  if (!browser) return;
  if (!invite.groupId || !invite.code) return;
  try {
    window.sessionStorage.setItem(KEY, JSON.stringify(invite));
  } catch {
    // ignore
  }
}

export function getPendingInvite(): PendingInvite | null {
  if (!browser) return null;
  try {
    const raw = window.sessionStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (
      parsed &&
      typeof parsed.groupId === 'string' &&
      typeof parsed.code === 'string' &&
      parsed.groupId &&
      parsed.code
    ) {
      return { groupId: parsed.groupId, code: parsed.code };
    }
    return null;
  } catch {
    return null;
  }
}

export function clearPendingInvite(): void {
  if (!browser) return;
  try {
    window.sessionStorage.removeItem(KEY);
  } catch {
    // ignore
  }
}

export function pendingInviteUrl(invite: PendingInvite): string {
  return `/r/${encodeURIComponent(invite.groupId)}/join/${encodeURIComponent(invite.code)}`;
}
