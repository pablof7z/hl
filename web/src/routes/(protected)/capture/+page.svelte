<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { NDKHighlight, NDKKind, NDKRelaySet, nip19 } from '@nostr-dev-kit/ndk';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';
  import {
    fetchLatestUserList,
    setBookmarkUrlPresence,
    bookmarkListHasUrl
  } from '$lib/ndk/lists';
  import {
    publishAndShareHighlight,
    highlightFromEvent,
    resolveUserHighlightRelayUrls
  } from '$lib/ndk/highlights';
  import { normalizeArtifactUrl, buildArtifactPreview } from '$lib/ndk/artifacts';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import type { OgMeta } from '../../api/og/+server';
  import type { RoomSummary } from '$lib/ndk/groups';
  import type { NDKEvent as NDKEventType } from '@nostr-dev-kit/ndk';

  // ─── State ────────────────────────────────────────────────────────────────

  type Stage = 'paste' | 'highlight';

  let stage = $state<Stage>('paste');
  let input = $state('');
  let inputError = $state('');

  // OG preview
  let ogLoading = $state(false);
  let ogMeta = $state<OgMeta | null>(null);
  let resolvedUrl = $state('');

  // Highlight fields
  let excerpt = $state('');
  let note = $state('');
  let selectedGroupId = $state('');

  // Publish state
  let publishing = $state(false);
  let publishError = $state('');
  let publishedHighlightId = $state('');

  // ─── NDK / auth ───────────────────────────────────────────────────────────

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  // Joined rooms subscription
  const membershipFeed = ndk.$subscribe(() => {
    if (!currentUser) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();
    for (const ev of membershipFeed.events) {
      const id = groupIdFromEvent(ev);
      if (id) ids.add(id);
    }
    return [...ids];
  });

  const metadataFeed = ndk.$subscribe(() => {
    if (!currentUser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupMetadata], '#d': membershipGroupIds, limit: Math.max(membershipGroupIds.length * 2, 32) }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const rooms: RoomSummary[] = $derived.by(() => {
    if (!currentUser) return [];
    return buildJoinedRooms(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events]);
  });

  // ─── Input detection ──────────────────────────────────────────────────────

  function isHttpUrl(value: string): boolean {
    try {
      const url = new URL(value.trim());
      return url.protocol === 'http:' || url.protocol === 'https:';
    } catch {
      return false;
    }
  }

  function isNostrUri(value: string): boolean {
    const trimmed = value.trim();
    if (trimmed.startsWith('nostr:')) return true;
    // also accept bare bech32 prefixes
    return /^(npub|nsec|note|naddr|nevent|nprofile)1[a-z0-9]+$/i.test(trimmed);
  }

  function extractBech32(value: string): string {
    const trimmed = value.trim();
    return trimmed.startsWith('nostr:') ? trimmed.slice('nostr:'.length) : trimmed;
  }

  function nostrRedirectPath(value: string): string | null {
    try {
      const bech32 = extractBech32(value);
      const decoded = nip19.decode(bech32);

      switch (decoded.type) {
        case 'naddr':
        case 'nevent':
        case 'note':
          return `/note/${bech32}`;
        case 'npub':
        case 'nprofile':
          return `/profile/${bech32}`;
        default:
          return null;
      }
    } catch {
      return null;
    }
  }

  // ─── Step 1: submit input ─────────────────────────────────────────────────

  async function handleSubmit() {
    inputError = '';
    const trimmed = input.trim();
    if (!trimmed) {
      inputError = 'Paste a URL or Nostr address first.';
      return;
    }

    if (isNostrUri(trimmed)) {
      const path = nostrRedirectPath(trimmed);
      if (path) {
        await goto(path);
        return;
      }
      inputError = 'Could not decode that Nostr address.';
      return;
    }

    if (isHttpUrl(trimmed)) {
      const normalized = normalizeArtifactUrl(trimmed);
      if (!normalized) {
        inputError = 'That URL could not be normalized.';
        return;
      }
      resolvedUrl = normalized;
      await fetchOg(normalized);
      return;
    }

    inputError = 'Enter an HTTP(S) URL or a Nostr address (npub, naddr, nevent…).';
  }

  async function fetchOg(url: string) {
    ogLoading = true;
    ogMeta = null;

    try {
      const res = await fetch(`/api/og?url=${encodeURIComponent(url)}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      ogMeta = await res.json() as OgMeta;
      stage = 'highlight';
    } catch {
      inputError = 'Could not fetch a preview for that URL. You can still continue without one.';
      ogMeta = null;
      stage = 'highlight';
    } finally {
      ogLoading = false;
    }
  }

  function reset() {
    stage = 'paste';
    input = '';
    inputError = '';
    ogMeta = null;
    resolvedUrl = '';
    excerpt = '';
    note = '';
    selectedGroupId = '';
    publishError = '';
    publishedHighlightId = '';
  }

  // ─── Step 3: Capture ──────────────────────────────────────────────────────

  async function handleCapture() {
    if (!currentUser) {
      publishError = 'Sign in before capturing.';
      return;
    }
    if (isReadOnly) {
      publishError = 'This signer is read-only.';
      return;
    }

    publishError = '';
    publishing = true;

    try {
      await ensureClientNdk();

      const trimmedExcerpt = excerpt.trim();

      if (!trimmedExcerpt) {
        // No excerpt — bookmark the URL
        const existingList = await fetchLatestUserList(ndk, 10003, currentUser.pubkey);
        const alreadyBookmarked = bookmarkListHasUrl(existingList, resolvedUrl);

        if (!alreadyBookmarked) {
          await setBookmarkUrlPresence(ndk, existingList, resolvedUrl, true);
        }

        publishedHighlightId = 'bookmarked';
        return;
      }

      // Has excerpt — publish kind:9802 highlight
      const preview = buildArtifactPreview({
        url: resolvedUrl,
        title: ogMeta?.title ?? '',
        description: ogMeta?.description ?? '',
        image: ogMeta?.image ?? '',
        author: ogMeta?.byline ?? ''
      });

      // Build a minimal ArtifactRecord (no room shareEventId needed for outbox publish)
      const artifact = {
        ...preview,
        groupId: selectedGroupId,
        shareEventId: '',
        pubkey: currentUser.pubkey,
        createdAt: Math.floor(Date.now() / 1000),
        note: ''
      };

      if (selectedGroupId) {
        const result = await publishAndShareHighlight(ndk, {
          groupId: selectedGroupId,
          artifact,
          quote: trimmedExcerpt,
          context: '',
          note: note.trim()
        });
        publishedHighlightId = result.highlight.eventId;
      } else {
        // Publish standalone to outbox only
        const relayUrls = await resolveUserHighlightRelayUrls(ndk, currentUser.pubkey);
        const event = new NDKHighlight(ndk);
        event.content = trimmedExcerpt;
        event.article = resolvedUrl;

        const trimmedNote = note.trim();
        if (trimmedNote) {
          event.tags.push(['comment', trimmedNote]);
        }

        await event.sign();
        await event.publish(NDKRelaySet.fromRelayUrls(relayUrls, ndk));

        const record = highlightFromEvent(event as NDKEventType);
        publishedHighlightId = record.eventId;
      }
    } catch (err) {
      publishError = err instanceof Error ? err.message : 'Could not publish.';
    } finally {
      publishing = false;
    }
  }

  // ─── Derived helpers ──────────────────────────────────────────────────────

  const selectedRoomName = $derived(
    rooms.find((r) => r.id === selectedGroupId)?.name ?? ''
  );

  const captureLabel = $derived(
    excerpt.trim()
      ? (selectedGroupId ? 'Publish highlight to room' : 'Publish highlight')
      : 'Bookmark URL'
  );

  const publishedNevent = $derived.by(() => {
    if (!publishedHighlightId || publishedHighlightId === 'bookmarked') return '';
    try {
      return nip19.neventEncode({ id: publishedHighlightId });
    } catch {
      return publishedHighlightId;
    }
  });
</script>

<svelte:head>
  <title>Capture — Highlighter</title>
</svelte:head>

<section class="grid gap-8 max-w-[56rem] mx-auto py-10 pb-16 px-4">

  <header class="grid gap-1">
    <h1 class="m-0 text-base-content text-2xl font-bold tracking-tight">Capture</h1>
    <p class="m-0 text-base-content/60 text-sm">Save a URL, excerpt, or Nostr address to your reading collection.</p>
  </header>

  {#if publishedHighlightId === 'bookmarked'}
    <div class="alert alert-success">
      <svg class="size-5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
      </svg>
      <div>
        <p class="m-0 font-semibold">Bookmarked</p>
        <p class="m-0 text-sm opacity-80">{resolvedUrl}</p>
      </div>
      <button type="button" class="btn btn-sm btn-ghost ml-auto" onclick={reset}>Capture another</button>
    </div>
  {:else if publishedHighlightId}
    <div class="alert alert-success">
      <svg class="size-5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
      </svg>
      <div>
        <p class="m-0 font-semibold">Captured</p>
        <a href="/note/{publishedNevent}" class="text-sm underline underline-offset-2">View highlight</a>
      </div>
      <button type="button" class="btn btn-sm btn-ghost ml-auto" onclick={reset}>Capture another</button>
    </div>
  {:else}
    <div class="grid gap-6 lg:grid-cols-[1fr_20rem] lg:items-start">

      <!-- ── Left column: main flow ── -->
      <div class="grid gap-5">

        <!-- Stage 1: Paste -->
        <div class="card bg-base-100 border border-base-300">
          <div class="card-body gap-4">
            <div class="flex items-center gap-2">
              <span class="badge badge-primary badge-sm font-mono text-[0.65rem]">1</span>
              <h2 class="m-0 text-base-content font-semibold text-base">Paste a URL or Nostr address</h2>
            </div>

            <div class="grid gap-2">
              <input
                type="url"
                class="input input-bordered w-full"
                placeholder="https://… or npub1… or naddr1…"
                bind:value={input}
                disabled={stage === 'highlight' || ogLoading}
                onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); void handleSubmit(); } }}
              />
              {#if inputError}
                <p class="m-0 text-error text-sm">{inputError}</p>
              {/if}
            </div>

            {#if stage === 'paste'}
              <button
                type="button"
                class="btn btn-primary w-fit"
                onclick={handleSubmit}
                disabled={ogLoading || !input.trim()}
              >
                {ogLoading ? 'Fetching preview…' : 'Preview'}
              </button>
            {:else}
              <button
                type="button"
                class="btn btn-ghost btn-sm w-fit text-base-content/50"
                onclick={reset}
              >
                Change URL
              </button>
            {/if}
          </div>
        </div>

        <!-- OG preview card (shown once loaded) -->
        {#if stage === 'highlight' && resolvedUrl}
          <div class="card bg-base-100 border border-base-300 overflow-hidden">
            {#if ogMeta?.image}
              <figure class="max-h-48 overflow-hidden">
                <img
                  src={ogMeta.image}
                  alt={ogMeta.title || 'Page preview'}
                  class="w-full object-cover"
                  loading="lazy"
                />
              </figure>
            {/if}
            <div class="card-body gap-1 py-4">
              {#if ogMeta?.siteName}
                <p class="m-0 text-xs font-semibold uppercase tracking-wider text-base-content/40">{ogMeta.siteName}</p>
              {/if}
              <h3 class="m-0 text-base-content font-semibold text-base leading-snug">
                {ogMeta?.title || resolvedUrl}
              </h3>
              {#if ogMeta?.byline}
                <p class="m-0 text-sm text-base-content/60">{ogMeta.byline}</p>
              {/if}
              {#if ogMeta?.description}
                <p class="m-0 text-sm text-base-content/60 leading-relaxed line-clamp-3 mt-1">{ogMeta.description}</p>
              {/if}
              <a
                href={resolvedUrl}
                target="_blank"
                rel="noopener noreferrer"
                class="mt-2 text-xs text-base-content/40 truncate hover:text-primary transition-colors"
              >{resolvedUrl}</a>
            </div>
          </div>

          <!-- Stage 2: Excerpt + note -->
          <div class="card bg-base-100 border border-base-300">
            <div class="card-body gap-4">
              <div class="flex items-center gap-2">
                <span class="badge badge-primary badge-sm font-mono text-[0.65rem]">2</span>
                <h2 class="m-0 text-base-content font-semibold text-base">Highlight a passage</h2>
              </div>

              <fieldset class="grid gap-1.5 border-none p-0 m-0">
                <legend class="text-xs font-semibold uppercase tracking-wider text-base-content/40 mb-0.5">
                  Passage to highlight <span class="font-normal normal-case tracking-normal opacity-60">(optional)</span>
                </legend>
                <textarea
                  class="textarea textarea-bordered w-full resize-y leading-relaxed"
                  rows="4"
                  maxlength="2000"
                  placeholder="Paste or type the excerpt you want to keep…"
                  bind:value={excerpt}
                ></textarea>
              </fieldset>

              <fieldset class="grid gap-1.5 border-none p-0 m-0">
                <legend class="text-xs font-semibold uppercase tracking-wider text-base-content/40 mb-0.5">
                  Add a note <span class="font-normal normal-case tracking-normal opacity-60">(optional)</span>
                </legend>
                <textarea
                  class="textarea textarea-bordered w-full resize-y"
                  rows="2"
                  maxlength="500"
                  placeholder="Your thoughts on this…"
                  bind:value={note}
                ></textarea>
              </fieldset>
            </div>
          </div>

          <!-- Stage 3: Capture CTA -->
          <div class="card bg-base-100 border border-base-300">
            <div class="card-body gap-4">
              <div class="flex items-center gap-2">
                <span class="badge badge-primary badge-sm font-mono text-[0.65rem]">3</span>
                <h2 class="m-0 text-base-content font-semibold text-base">Capture</h2>
              </div>

              {#if publishError}
                <div class="alert alert-error text-sm py-2">
                  <span>{publishError}</span>
                </div>
              {/if}

              <button
                type="button"
                class="btn btn-primary"
                onclick={handleCapture}
                disabled={publishing || isReadOnly}
              >
                {publishing ? 'Publishing…' : captureLabel}
              </button>

              {#if isReadOnly}
                <p class="m-0 text-sm text-warning">This signer is read-only. Switch to a writable signer to publish.</p>
              {/if}
            </div>
          </div>
        {/if}

      </div>

      <!-- ── Right column: room picker (only in highlight stage) ── -->
      {#if stage === 'highlight'}
        <aside class="card bg-base-100 border border-base-300 lg:sticky lg:top-6">
          <div class="card-body gap-4">
            <h3 class="m-0 text-base-content font-semibold text-sm">Share to a room</h3>
            <p class="m-0 text-xs text-base-content/50 leading-relaxed">
              Optionally share this capture into one of your rooms. Leave unset to publish to your outbox only.
            </p>

            {#if rooms.length === 0}
              <p class="m-0 text-xs text-base-content/40 italic">No rooms loaded yet. Join or create a room first.</p>
              <div class="flex flex-wrap gap-2">
                <a href="/discover" class="btn btn-xs btn-ghost btn-outline">Browse rooms</a>
                <a href="/r/create" class="btn btn-xs btn-ghost btn-outline">Create a room</a>
              </div>
            {:else}
              <select class="select select-bordered select-sm w-full" bind:value={selectedGroupId}>
                <option value="">No room — publish to outbox</option>
                {#each rooms as room (room.id)}
                  <option value={room.id}>{room.name}</option>
                {/each}
              </select>
              {#if selectedRoomName}
                <p class="m-0 text-xs text-primary font-medium">Will share into: {selectedRoomName}</p>
              {/if}
            {/if}
          </div>
        </aside>
      {/if}

    </div>
  {/if}

</section>
