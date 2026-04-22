import { browser } from '$app/environment';

/**
 * Client-side registry of invite codes minted by the current admin.
 *
 * The relay stores the code itself (so it can validate joins) but not the
 * human label, the person it was created for, or when it was minted by whom.
 * That metadata lives here in localStorage, keyed by the signer's pubkey so
 * codes minted on one device don't leak labels onto another user's list.
 *
 * This is ephemeral by design. If the admin clears storage, their codes still
 * work — they just lose the labels. Losing labels is not a security issue.
 */

export type InviteRecord = {
  code: string;
  label: string;
  createdAt: number;
  createdBy: string;
};

const STORAGE_KEY_PREFIX = 'hl:invites:';

function storageKey(ownerPubkey: string, groupId: string): string {
  return `${STORAGE_KEY_PREFIX}${ownerPubkey}:${groupId}`;
}

function readAll(ownerPubkey: string, groupId: string): InviteRecord[] {
  if (!browser) return [];
  if (!ownerPubkey || !groupId) return [];
  try {
    const raw = window.localStorage.getItem(storageKey(ownerPubkey, groupId));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isInviteRecord);
  } catch {
    return [];
  }
}

function writeAll(ownerPubkey: string, groupId: string, records: InviteRecord[]): void {
  if (!browser) return;
  if (!ownerPubkey || !groupId) return;
  try {
    window.localStorage.setItem(
      storageKey(ownerPubkey, groupId),
      JSON.stringify(records)
    );
  } catch {
    // Quota or private-mode errors — silently ignore; labels are cosmetic.
  }
}

function isInviteRecord(value: unknown): value is InviteRecord {
  if (!value || typeof value !== 'object') return false;
  const v = value as Partial<InviteRecord>;
  return (
    typeof v.code === 'string' &&
    typeof v.label === 'string' &&
    typeof v.createdAt === 'number' &&
    typeof v.createdBy === 'string'
  );
}

export function listInvites(ownerPubkey: string, groupId: string): InviteRecord[] {
  return readAll(ownerPubkey, groupId).sort((a, b) => b.createdAt - a.createdAt);
}

export function recordInvite(
  ownerPubkey: string,
  groupId: string,
  entry: { code: string; label?: string }
): InviteRecord {
  const record: InviteRecord = {
    code: entry.code,
    label: entry.label?.trim() ?? '',
    createdAt: Math.floor(Date.now() / 1000),
    createdBy: ownerPubkey
  };
  const records = readAll(ownerPubkey, groupId);
  records.push(record);
  writeAll(ownerPubkey, groupId, records);
  return record;
}

export function recordInvites(
  ownerPubkey: string,
  groupId: string,
  entries: Array<{ code: string; label?: string }>
): InviteRecord[] {
  const now = Math.floor(Date.now() / 1000);
  const fresh: InviteRecord[] = entries.map((entry) => ({
    code: entry.code,
    label: entry.label?.trim() ?? '',
    createdAt: now,
    createdBy: ownerPubkey
  }));
  const existing = readAll(ownerPubkey, groupId);
  writeAll(ownerPubkey, groupId, [...existing, ...fresh]);
  return fresh;
}

export function deleteInvite(
  ownerPubkey: string,
  groupId: string,
  code: string
): void {
  const records = readAll(ownerPubkey, groupId).filter((r) => r.code !== code);
  writeAll(ownerPubkey, groupId, records);
}

export function updateInviteLabel(
  ownerPubkey: string,
  groupId: string,
  code: string,
  label: string
): void {
  const records = readAll(ownerPubkey, groupId);
  const next = records.map((r) => (r.code === code ? { ...r, label: label.trim() } : r));
  writeAll(ownerPubkey, groupId, next);
}
