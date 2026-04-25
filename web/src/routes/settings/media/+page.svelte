<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKBlossomList, NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  // Read Blossom list from session
  const blossomEvent = $derived(ndk.$sessions?.getSessionEvent(NDKKind.BlossomList));

  // Local editable copy of servers
  let servers = $state<string[]>([]);
  let loaded = $state(false);

  $effect(() => {
    if (!browser || loaded) return;
    const event = blossomEvent;
    if (event instanceof NDKBlossomList) {
      servers = [...event.servers];
    } else if (event) {
      servers = event.tags
        .filter((tag) => tag[0] === 'server' && tag[1])
        .map((tag) => tag[1]);
    }
    loaded = true;
  });

  // Add server form
  let addUrl = $state('');
  let addValidating = $state(false);
  let addError = $state('');
  let saving = $state(false);

  function normalizeServerUrl(raw: string): string | null {
    const trimmed = raw.trim().replace(/\/$/, '');
    if (!trimmed) return null;
    const withProto = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
    try {
      const url = new URL(withProto);
      if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
      return url.origin;
    } catch {
      return null;
    }
  }

  async function validateServer(url: string): Promise<boolean> {
    try {
      const response = await fetch(`${url}/`, { method: 'HEAD', signal: AbortSignal.timeout(5000) });
      return response.ok;
    } catch {
      return false;
    }
  }

  async function addServer() {
    if (addValidating || saving || isReadOnly) return;

    const normalized = normalizeServerUrl(addUrl);
    if (!normalized) {
      addError = 'Enter a valid https:// URL';
      return;
    }

    if (servers.includes(normalized)) {
      addError = 'Server already in your list';
      return;
    }

    addError = '';
    addValidating = true;

    const reachable = await validateServer(normalized);
    addValidating = false;

    if (!reachable) {
      addError = "Server didn't respond — you can still add it with the force button below";
      return;
    }

    await commitAdd(normalized);
  }

  async function forceAdd() {
    if (saving || isReadOnly) return;
    const normalized = normalizeServerUrl(addUrl);
    if (!normalized) {
      addError = 'Enter a valid https:// URL';
      return;
    }
    if (servers.includes(normalized)) {
      addError = 'Server already in your list';
      return;
    }
    addError = '';
    await commitAdd(normalized);
  }

  async function commitAdd(url: string) {
    servers = [...servers, url];
    addUrl = '';
    await saveServers();
  }

  async function removeServer(url: string) {
    if (isReadOnly) return;
    servers = servers.filter((s) => s !== url);
    await saveServers();
  }

  async function moveUp(index: number) {
    if (index === 0 || isReadOnly) return;
    const next = [...servers];
    [next[index - 1], next[index]] = [next[index], next[index - 1]];
    servers = next;
    await saveServers();
  }

  async function moveDown(index: number) {
    if (index === servers.length - 1 || isReadOnly) return;
    const next = [...servers];
    [next[index], next[index + 1]] = [next[index + 1], next[index]];
    servers = next;
    await saveServers();
  }

  async function saveServers() {
    if (!currentUser || isReadOnly) return;

    saving = true;
    try {
      const existing = blossomEvent;
      const list =
        existing instanceof NDKBlossomList
          ? NDKBlossomList.from(existing)
          : existing
            ? NDKBlossomList.from(existing)
            : new NDKBlossomList(ndk);

      list.servers = servers;
      await list.publishReplaceable();
      ndk.$sessions?.current?.events.set(NDKKind.BlossomList, list);
    } finally {
      saving = false;
    }
  }

  function handleAddKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void addServer();
    }
  }

  const showForce = $derived(
    addError.includes("didn't respond") && normalizeServerUrl(addUrl) !== null
  );
</script>

<svelte:head>
  <title>Media — Settings — Highlighter</title>
</svelte:head>

<div class="grid gap-8">
  <div class="grid gap-1">
    <h1 class="m-0 text-xl font-bold text-base-content">Media</h1>
    <p class="m-0 text-sm text-base-content/60">
      Blossom servers store your uploaded images and files. The first reachable server is used for uploads.
    </p>
  </div>

  <!-- Server list -->
  <div class="grid gap-2">
    {#if !loaded}
      <div class="flex items-center gap-2 text-sm text-base-content/50">
        <span class="loading loading-spinner loading-xs"></span>
        Loading…
      </div>
    {:else if servers.length === 0}
      <div class="rounded-xl border border-dashed border-base-300 p-8 text-center text-sm text-base-content/50">
        No Blossom servers configured. Add one below.
      </div>
    {:else}
      {#each servers as server, i (server)}
        <div class="server-row">
          <div class="server-avatar" aria-hidden="true">
            <svg class="size-4 text-base-content/40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3.375 19.5h17.25m-17.25 0a1.125 1.125 0 0 1-1.125-1.125M3.375 19.5h1.5C5.496 19.5 6 18.996 6 18.375m-3.75.125H2.25m0 0A1.125 1.125 0 0 1 1.125 18V5.625M21.75 19.5h-1.5a1.5 1.5 0 0 1-1.5-1.5V5.625m3.75 13.875H22.875m0 0A1.125 1.125 0 0 0 24 18V5.625m-22.875 0A1.125 1.125 0 0 1 2.25 4.5h19.5A1.125 1.125 0 0 1 22.875 5.625m0 0v12.75" />
            </svg>
          </div>

          <div class="server-info">
            {#if i === 0}
              <span class="server-primary-badge">Primary</span>
            {/if}
            <span class="server-url">{server}</span>
          </div>

          {#if !isReadOnly}
            <div class="server-actions">
              <button
                class="btn btn-ghost btn-xs"
                onclick={() => void moveUp(i)}
                disabled={i === 0 || saving}
                title="Move up"
                aria-label="Move {server} up"
              >
                <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="m5 15 7-7 7 7" />
                </svg>
              </button>
              <button
                class="btn btn-ghost btn-xs"
                onclick={() => void moveDown(i)}
                disabled={i === servers.length - 1 || saving}
                title="Move down"
                aria-label="Move {server} down"
              >
                <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="m19 9-7 7-7-7" />
                </svg>
              </button>
              <button
                class="btn btn-ghost btn-xs text-error hover:bg-error/10"
                onclick={() => void removeServer(server)}
                disabled={saving}
                title="Remove"
                aria-label="Remove {server}"
              >
                <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M6 18 18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <!-- Add server form -->
  {#if !isReadOnly}
    <div class="grid gap-3">
      <h2 class="m-0 text-sm font-semibold uppercase tracking-wider text-base-content/50">Add server</h2>
      <div class="grid gap-2">
        <div class="flex gap-2">
          <input
            class="input input-sm flex-1 font-mono text-sm"
            placeholder="https://blossom.example.com"
            bind:value={addUrl}
            onkeydown={handleAddKeydown}
            type="url"
            autocomplete="off"
            autocapitalize="none"
            autocorrect="off"
            spellcheck="false"
            disabled={addValidating || saving}
          />
          <button
            class="btn btn-sm btn-primary"
            onclick={() => void addServer()}
            disabled={addValidating || saving || !addUrl.trim()}
          >
            {addValidating ? 'Checking…' : 'Add'}
          </button>
        </div>

        {#if addError}
          <div class="flex items-start gap-2">
            <p class="m-0 text-xs text-warning flex-1">{addError}</p>
            {#if showForce}
              <button class="btn btn-xs btn-outline btn-warning shrink-0" onclick={() => void forceAdd()}>
                Add anyway
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if saving}
    <p class="m-0 flex items-center gap-1.5 text-xs text-base-content/60">
      <span class="loading loading-spinner loading-xs"></span>
      Saving…
    </p>
  {/if}

  <p class="m-0 text-xs text-base-content/40">
    Stored as kind:10063 (Blossom server list). Files are uploaded to the first reachable server. Drag or use arrows to reorder priority.
  </p>
</div>

<style>
  .server-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.625rem 0.75rem;
    border-radius: 0.75rem;
    border: 1px solid var(--color-base-200, oklch(93% 0.01 250));
    background: var(--color-base-100, #fff);
  }

  .server-avatar {
    width: 2rem;
    height: 2rem;
    border-radius: 0.5rem;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-base-200, oklch(93% 0.01 250));
  }

  .server-info {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .server-primary-badge {
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-primary, oklch(50% 0.2 260));
    background: color-mix(in srgb, var(--color-primary, oklch(50% 0.2 260)) 12%, transparent);
    padding: 0.1rem 0.4rem;
    border-radius: 9999px;
    flex-shrink: 0;
  }

  .server-url {
    font-size: 0.8125rem;
    font-family: var(--font-mono, monospace);
    color: var(--color-base-content);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .server-actions {
    display: flex;
    align-items: center;
    gap: 0.125rem;
    flex-shrink: 0;
  }
</style>
