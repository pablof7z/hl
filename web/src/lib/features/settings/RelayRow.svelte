<script lang="ts">
  import { createRelayInfo } from '@nostr-dev-kit/svelte';
  import { NDKRelayStatus } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import type { RelayRole } from '$lib/ndk/app-data';

  interface Props {
    url: string;
    markers: { read: boolean; write: boolean };
    roles: RelayRole[];
    status: NDKRelayStatus | undefined;
    isReconnecting: boolean;
    isReadOnly: boolean;
    onToggleRead: () => void;
    onToggleWrite: () => void;
    onToggleRole: (role: RelayRole) => void;
    onRemove: () => void;
    onReconnect: () => void;
  }

  let {
    url,
    markers,
    roles,
    status,
    isReconnecting,
    isReadOnly,
    onToggleRead,
    onToggleWrite,
    onToggleRole,
    onRemove,
    onReconnect
  }: Props = $props();

  const relayInfo = createRelayInfo(() => ({ relayUrl: url }), ndk);

  function hostnameFromUrl(rawUrl: string): string {
    try {
      return new URL(rawUrl).hostname;
    } catch {
      return rawUrl.replace(/^wss?:\/\//, '');
    }
  }

  const hostname = $derived(hostnameFromUrl(url));
  const displayName = $derived(relayInfo.nip11?.name?.trim() || hostname);

  function statusDotClass(s: NDKRelayStatus | undefined): string {
    if (s === NDKRelayStatus.CONNECTED || s === NDKRelayStatus.AUTHENTICATED) return 'bg-success';
    if (
      s === NDKRelayStatus.CONNECTING ||
      s === NDKRelayStatus.RECONNECTING ||
      s === NDKRelayStatus.AUTHENTICATING
    ) return 'bg-warning animate-pulse';
    if (s === undefined) return 'bg-base-300';
    return 'bg-error';
  }

  function statusTitle(s: NDKRelayStatus | undefined): string {
    if (s === undefined) return 'Unknown';
    return NDKRelayStatus[s] ?? 'Unknown';
  }

  // hue from url for monogram fallback
  function hueColor(rawUrl: string): string {
    const seed = [...rawUrl].reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
    const hue = seed % 360;
    return `hsl(${hue} 55% 55%)`;
  }


</script>

<div class="relay-row">
  <!-- Avatar -->
  <div class="relay-avatar" style="background-color: {relayInfo.nip11?.icon ? 'transparent' : hueColor(url)}">
    {#if relayInfo.nip11?.icon}
      <img src={relayInfo.nip11.icon} alt="" class="size-full object-cover" loading="lazy" />
    {:else}
      <span class="text-white font-semibold text-xs">
        {hostname.charAt(0).toUpperCase()}
      </span>
    {/if}
  </div>

  <!-- Info -->
  <div class="relay-info">
    <div class="relay-info-top">
      <!-- Status dot -->
      <span
        class="relay-status-dot {statusDotClass(status)} {isReconnecting ? 'animate-pulse' : ''}"
        title={isReconnecting ? 'Reconnecting…' : statusTitle(status)}
      ></span>
      <span class="relay-name">{displayName}</span>
    </div>
    <span class="relay-url">{hostname}</span>
    <!-- Role chips -->
    <div class="relay-chips">
      <button
        class="relay-chip"
        class:relay-chip-on={markers.read}
        onclick={onToggleRead}
        disabled={isReadOnly}
        title={markers.read ? 'Disable Read' : 'Enable Read'}
      >Read</button>
      <button
        class="relay-chip"
        class:relay-chip-on={markers.write}
        onclick={onToggleWrite}
        disabled={isReadOnly}
        title={markers.write ? 'Disable Write' : 'Enable Write'}
      >Write</button>
      <button
        class="relay-chip"
        class:relay-chip-on={roles.includes('rooms-host')}
        onclick={() => onToggleRole('rooms-host')}
        disabled={isReadOnly}
        title="Rooms host relay — routes NIP-29 group traffic"
      >Rooms</button>
      <button
        class="relay-chip"
        class:relay-chip-on={roles.includes('indexer')}
        onclick={() => onToggleRole('indexer')}
        disabled={isReadOnly}
        title="Indexer relay — outbox-model bootstrap pool"
      >Indexer</button>
      <button
        class="relay-chip"
        class:relay-chip-on={roles.includes('search')}
        onclick={() => onToggleRole('search')}
        disabled={isReadOnly}
        title="Search relay — NIP-50 search queries"
      >Search</button>
    </div>
  </div>

  <!-- Actions -->
  <div class="relay-actions">
    {#if isReconnecting}
      <span class="loading loading-spinner loading-xs text-base-content/40"></span>
    {:else if status !== NDKRelayStatus.CONNECTED && status !== NDKRelayStatus.AUTHENTICATED}
      <button
        class="btn btn-ghost btn-xs"
        onclick={onReconnect}
        title="Reconnect"
        aria-label="Reconnect {hostname}"
      >
        <svg class="size-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M21 2v6h-6" />
          <path d="M21 13a9 9 0 1 1-3-7.7L21 8" />
        </svg>
      </button>
    {/if}

    {#if !isReadOnly}
      <button
        class="btn btn-ghost btn-xs text-error hover:bg-error/10"
        onclick={onRemove}
        title="Remove {hostname}"
        aria-label="Remove {hostname}"
      >
        <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M6 18 18 6M6 6l12 12" />
        </svg>
      </button>
    {/if}
  </div>
</div>

<style>
  .relay-row {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.75rem;
    border-radius: 0.75rem;
    border: 1px solid var(--color-base-200, oklch(93% 0.01 250));
    background: var(--color-base-100, #fff);
    transition: border-color 0.15s;
  }

  .relay-row:hover {
    border-color: var(--color-base-300, oklch(86% 0.01 250));
  }

  .relay-avatar {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 0.5rem;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .relay-info {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 0.2rem;
  }

  .relay-info-top {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .relay-status-dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 9999px;
    flex-shrink: 0;
  }

  .relay-name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-base-content);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .relay-url {
    font-size: 0.75rem;
    color: var(--color-base-content);
    opacity: 0.5;
    font-family: var(--font-mono, monospace);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .relay-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.25rem;
  }

  .relay-chip {
    padding: 0.125rem 0.5rem;
    border-radius: 9999px;
    border: 1px solid var(--color-base-300, oklch(86% 0.01 250));
    background: transparent;
    color: var(--color-base-content);
    opacity: 0.4;
    font-size: 0.7rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s, opacity 0.12s, border-color 0.12s, color 0.12s;
    line-height: 1.4;
  }

  .relay-chip:hover:not(:disabled) {
    opacity: 0.7;
    border-color: var(--color-primary, oklch(50% 0.2 260));
  }

  .relay-chip:disabled {
    cursor: default;
  }

  .relay-chip-on {
    background: color-mix(in srgb, var(--color-primary, oklch(50% 0.2 260)) 15%, transparent);
    border-color: color-mix(in srgb, var(--color-primary, oklch(50% 0.2 260)) 40%, transparent);
    color: var(--color-primary, oklch(50% 0.2 260));
    opacity: 1;
  }

  .relay-actions {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex-shrink: 0;
  }
</style>
