<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKRelayList, NDKRelayStatus, NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import {
    probeRelayNip11,
    type Nip11ProbeResult
  } from '$lib/ndk/relay-probe';
  import {
    fetchAppData,
    parseRelayRoles,
    publishRelayRoles,
    APP_DATA_DTAG_RELAY_ROLES,
    type RelayRole,
    type RelayRoleMap
  } from '$lib/ndk/app-data';
  import RelayRow from '$lib/features/settings/RelayRow.svelte';

  // ── State ─────────────────────────────────────────────────────
  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  // NIP-65 relay list from session (Map<url, {read, write}>)
  const sessionRelayMap = $derived(ndk.$sessions?.relayList ?? new Map<string, { read: boolean; write: boolean }>());
  const relayUrls = $derived([...sessionRelayMap.keys()]);

  // NIP-78 relay roles (rooms-host / indexer / search)
  let roleMap = $state<RelayRoleMap>({});

  // Add relay form
  let addUrl = $state('');
  let addProbing = $state(false);
  let addProbeResult = $state<Nip11ProbeResult | null>(null);
  let addProbeTimer: ReturnType<typeof setTimeout> | null = null;
  let addError = $state('');
  let addSaving = $state(false);

  // Per-relay reconnecting state
  let reconnectingUrls = $state<Set<string>>(new Set());

  // ── Load NIP-78 roles ─────────────────────────────────────────
  $effect(() => {
    const pubkey = currentUser?.pubkey;
    if (!pubkey || !browser) return;

    void fetchAppData(ndk, pubkey, APP_DATA_DTAG_RELAY_ROLES).then((event) => {
      roleMap = parseRelayRoles(event);
    });
  });

  // ── NIP-65 helpers ────────────────────────────────────────────
  function buildRelayList(map: Map<string, { read: boolean; write: boolean }>): NDKRelayList {
    const list = new NDKRelayList(ndk);
    const tags: string[][] = [];
    for (const [url, { read, write }] of map) {
      if (read && write) {
        tags.push(['r', url]);
      } else if (read) {
        tags.push(['r', url, 'read']);
      } else if (write) {
        tags.push(['r', url, 'write']);
      }
    }
    list.tags = tags;
    return list;
  }

  async function saveRelayList(map: Map<string, { read: boolean; write: boolean }>) {
    const list = buildRelayList(map);
    await list.sign();
    await list.publishReplaceable();
    ndk.$sessions?.current?.events.set(NDKKind.RelayList, list);
  }

  // ── Toggle read/write markers ─────────────────────────────────
  async function toggleMarker(url: string, marker: 'read' | 'write') {
    if (isReadOnly || !currentUser) return;
    const next = new Map(sessionRelayMap);
    const current = next.get(url) ?? { read: false, write: false };
    next.set(url, { ...current, [marker]: !current[marker] });
    await saveRelayList(next);
  }

  // ── Toggle special roles ─────────────────────────────────────
  async function toggleRole(url: string, role: RelayRole) {
    if (isReadOnly || !currentUser) return;

    const current = roleMap[url] ?? [];
    const updated: RelayRole[] = current.includes(role)
      ? current.filter((r) => r !== role)
      : [...current, role];

    const nextMap: RelayRoleMap = { ...roleMap };
    if (updated.length === 0) {
      delete nextMap[url];
    } else {
      nextMap[url] = updated;
    }

    roleMap = nextMap;
    await publishRelayRoles(ndk, nextMap);
  }

  // ── Remove relay ─────────────────────────────────────────────
  async function removeRelay(url: string) {
    if (isReadOnly || !currentUser) return;

    const next = new Map(sessionRelayMap);
    next.delete(url);
    await saveRelayList(next);

    const nextRoles: RelayRoleMap = { ...roleMap };
    if (url in nextRoles) {
      delete nextRoles[url];
      roleMap = nextRoles;
      await publishRelayRoles(ndk, nextRoles);
    }
  }

  // ── Add relay ─────────────────────────────────────────────────
  function normalizeWsUrl(raw: string): string {
    const trimmed = raw.trim();
    if (/^wss?:\/\//i.test(trimmed)) return trimmed;
    return `wss://${trimmed}`;
  }

  function scheduleProbe() {
    if (addProbeTimer) clearTimeout(addProbeTimer);
    addProbeResult = null;
    addError = '';
    const url = normalizeWsUrl(addUrl);
    if (!url.startsWith('wss://') && !url.startsWith('ws://')) return;

    addProbeTimer = setTimeout(async () => {
      addProbing = true;
      addProbeResult = await probeRelayNip11(url);
      addProbing = false;
    }, 600);
  }

  async function addRelay() {
    if (addSaving || isReadOnly || !currentUser) return;

    const url = normalizeWsUrl(addUrl);
    if (!url.startsWith('wss://') && !url.startsWith('ws://')) {
      addError = 'URL must start with wss:// or ws://';
      return;
    }

    if (sessionRelayMap.has(url)) {
      addError = 'Relay already in your list';
      return;
    }

    addSaving = true;
    addError = '';

    try {
      if (!addProbeResult) {
        addProbeResult = await probeRelayNip11(url);
      }

      const next = new Map(sessionRelayMap);
      next.set(url, { read: true, write: true });
      await saveRelayList(next);
      addUrl = '';
      addProbeResult = null;
    } catch {
      addError = "Couldn't save relay list";
    } finally {
      addSaving = false;
    }
  }

  function handleAddKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void addRelay();
    }
  }

  // ── Reconnect ─────────────────────────────────────────────────
  async function reconnectAll() {
    const pool = ndk.pool;
    const urlList = [...pool.relays.keys()];
    for (const url of urlList) {
      const relay = pool.relays.get(url);
      if (!relay) continue;
      const s = relay.status;
      if (
        s === NDKRelayStatus.DISCONNECTED ||
        s === NDKRelayStatus.DISCONNECTING ||
        s === NDKRelayStatus.FLAPPING
      ) {
        reconnectingUrls = new Set([...reconnectingUrls, url]);
        void relay.connect().then(() => {
          reconnectingUrls = new Set([...reconnectingUrls].filter((u) => u !== url));
        });
      }
    }
  }

  async function reconnectOne(url: string) {
    const relay = ndk.pool.relays.get(url);
    if (!relay) return;
    reconnectingUrls = new Set([...reconnectingUrls, url]);
    try {
      await relay.connect();
    } finally {
      reconnectingUrls = new Set([...reconnectingUrls].filter((u) => u !== url));
    }
  }

  // ── Status helpers ────────────────────────────────────────────
  function relayStatus(url: string): NDKRelayStatus | undefined {
    return ndk.pool.relays.get(url)?.status;
  }

  const connectedCount = $derived.by(() => {
    let count = 0;
    for (const url of relayUrls) {
      const s = relayStatus(url);
      if (s === NDKRelayStatus.CONNECTED || s === NDKRelayStatus.AUTHENTICATED) count++;
    }
    return count;
  });

  const hasNoWrite = $derived(
    relayUrls.length > 0 && relayUrls.every((url) => !sessionRelayMap.get(url)?.write)
  );
</script>

<svelte:head>
  <title>Network — Settings — Highlighter</title>
</svelte:head>

<div class="grid gap-8">
  <div class="grid gap-1">
    <h1 class="m-0 text-xl font-bold text-base-content">Network</h1>
    <p class="m-0 text-sm text-base-content/60">
      {#if relayUrls.length > 0}
        {connectedCount} of {relayUrls.length} relay{relayUrls.length !== 1 ? 's' : ''} connected
      {:else}
        No relays configured
      {/if}
    </p>
  </div>

  {#if hasNoWrite}
    <div class="alert alert-warning py-3 text-sm">
      <svg class="size-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
      </svg>
      <span>No write relays — your events won't reach anyone. Enable Write on at least one relay.</span>
    </div>
  {/if}

  <!-- Relay list -->
  <div class="grid gap-2">
    {#each relayUrls as url (url)}
      <RelayRow
        {url}
        markers={sessionRelayMap.get(url) ?? { read: false, write: false }}
        roles={roleMap[url] ?? []}
        status={relayStatus(url)}
        isReconnecting={reconnectingUrls.has(url)}
        {isReadOnly}
        onToggleRead={() => void toggleMarker(url, 'read')}
        onToggleWrite={() => void toggleMarker(url, 'write')}
        onToggleRole={(role) => void toggleRole(url, role)}
        onRemove={() => void removeRelay(url)}
        onReconnect={() => void reconnectOne(url)}
      />
    {:else}
      <div class="rounded-xl border border-dashed border-base-300 p-8 text-center text-sm text-base-content/50">
        No relays in your NIP-65 list yet. Add one below.
      </div>
    {/each}
  </div>

  <!-- Add relay form -->
  {#if !isReadOnly}
    <div class="grid gap-3">
      <h2 class="m-0 text-sm font-semibold uppercase tracking-wider text-base-content/50">Add relay</h2>
      <div class="grid gap-2">
        <div class="flex gap-2">
          <input
            class="input input-sm flex-1 font-mono text-sm"
            placeholder="wss://relay.example.com"
            bind:value={addUrl}
            oninput={scheduleProbe}
            onkeydown={handleAddKeydown}
            autocomplete="off"
            autocapitalize="none"
            autocorrect="off"
            spellcheck="false"
            disabled={addSaving}
          />
          <button
            class="btn btn-sm btn-primary"
            onclick={() => void addRelay()}
            disabled={addSaving || !addUrl.trim()}
          >
            {addSaving ? 'Adding…' : 'Add'}
          </button>
        </div>

        {#if addProbing}
          <p class="m-0 flex items-center gap-1.5 text-xs text-base-content/60">
            <span class="loading loading-spinner loading-xs"></span>
            Checking relay…
          </p>
        {:else if addProbeResult?.document}
          <p class="m-0 text-xs text-success">
            {addProbeResult.document.name ?? 'Relay reachable'}
            {#if addProbeResult.document.description}
              — {addProbeResult.document.description}
            {/if}
          </p>
        {:else if addProbeResult?.error}
          <p class="m-0 text-xs text-warning">
            Couldn't reach relay: {addProbeResult.error} — you can still add it.
          </p>
        {/if}

        {#if addError}
          <p class="m-0 text-xs text-error">{addError}</p>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Actions -->
  <div class="flex flex-wrap gap-2">
    <button
      class="btn btn-sm btn-outline"
      onclick={() => void reconnectAll()}
      disabled={relayUrls.length === 0}
    >
      <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M21 2v6h-6" />
        <path d="M21 13a9 9 0 1 1-3-7.7L21 8" />
      </svg>
      Reconnect all
    </button>
  </div>

  <p class="m-0 text-xs text-base-content/40">
    Read and Write relays are published as kind:10002 (NIP-65). Chips update that event immediately. Rooms, Indexer, and Search roles are stored in a private kind:30078 event.
  </p>
</div>
