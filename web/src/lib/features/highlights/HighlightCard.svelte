<script lang="ts">
  import { formatPodcastClock } from '$lib/features/podcasts/format';
  import DiscussionPanel from '$lib/features/discussions/DiscussionPanel.svelte';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { artifactPath, buildFallbackNostrUrl } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import {
    highlightPath,
    shareHighlightToRoom,
    type HydratedHighlight,
    type HighlightShareRecord
  } from '$lib/ndk/highlights';
  import type { RoomSummary } from '$lib/ndk/groups';

  let {
    highlight,
    artifact = undefined,
    rooms = [],
    showShareControl = false,
    showDiscussAction = false,
    groupId = '',
    seekTo = undefined
  }: {
    highlight: HydratedHighlight;
    artifact?: ArtifactRecord | undefined;
    rooms?: RoomSummary[];
    showShareControl?: boolean;
    showDiscussAction?: boolean;
    groupId?: string;
    seekTo?: ((seconds: number) => void) | undefined;
  } = $props();

  let showThread = $state(false);
  const canDiscuss = $derived(Boolean(showDiscussAction && groupId && highlight.eventId));

  let optimisticShares = $state<HighlightShareRecord[]>([]);
  let selectedGroupId = $state('');
  let sharing = $state(false);
  let shareError = $state('');
  let shareStatus = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const allShares = $derived(
    [...highlight.shares, ...optimisticShares].toSorted(
      (left, right) => (right.createdAt ?? 0) - (left.createdAt ?? 0)
    )
  );
  const sharedGroupIds = $derived(new Set(allShares.map((share) => share.groupId)));
  const shareableRooms = $derived(
    rooms.filter((room) => !sharedGroupIds.has(room.id))
  );
  const canShareAgain = $derived(
    Boolean(showShareControl && currentUser && !isReadOnly && selectedGroupId && !sharing)
  );
  const primaryShare = $derived(allShares[0]);
  const excerptSegments = $derived(buildExcerptSegments(highlight.context, highlight.quote));
  const clipLabel = $derived.by(() => {
    if (highlight.clipStartSeconds == null) return '';
    const startLabel = formatPodcastClock(highlight.clipStartSeconds);
    const endLabel =
      highlight.clipEndSeconds != null ? formatPodcastClock(highlight.clipEndSeconds) : '';
    return endLabel ? `${startLabel}-${endLabel}` : startLabel;
  });
  const createdLabel = $derived(
    highlight.createdAt
      ? new Intl.DateTimeFormat('en', {
          month: 'short',
          day: 'numeric',
          year: 'numeric'
        }).format(new Date(highlight.createdAt * 1000))
      : ''
  );
  const artifactPageHref = $derived(
    artifact?.groupId && artifact.id ? artifactPath(artifact.groupId, artifact.id) : ''
  );
  const sourceTitle = $derived.by(() => {
    if (artifact?.title) return artifact.title;
    if (highlight.sourceUrl) return highlight.sourceUrl.replace(/^https?:\/\//, '');
    if (highlight.artifactAddress) return 'Nostr article';
    if (highlight.eventReference) return 'Referenced event';
    return 'Unknown source';
  });
  const canSeek = $derived(Boolean(seekTo && highlight.clipStartSeconds != null));
  const sourceHref = $derived.by(() => {
    if (artifact?.url) return artifact.url;
    if (highlight.sourceUrl) return highlight.sourceUrl;
    if (highlight.artifactAddress) return buildFallbackNostrUrl(highlight.artifactAddress);
    return '';
  });
  const sourceMeta = $derived.by(() => {
    const values: string[] = [];

    if (artifact?.source) values.push(artifact.source);
    if (artifact?.domain) values.push(artifact.domain);
    if (!artifact && highlight.artifactAddress) values.push('nostr');

    return [...new Set(values.filter(Boolean))];
  });

  $effect(() => {
    if (!selectedGroupId && shareableRooms.length > 0) {
      selectedGroupId = shareableRooms[0].id;
    }
  });

  async function handleShareAgain() {
    if (!canShareAgain) return;

    sharing = true;
    shareError = '';
    shareStatus = '';

    try {
      await ensureClientNdk();

      const result = await shareHighlightToRoom(ndk, {
        groupId: selectedGroupId,
        highlight
      });

      if (!result.existing) {
        optimisticShares = [result.share, ...optimisticShares];
      }

      const selectedRoom =
        rooms.find((room) => room.id === selectedGroupId)?.name ?? selectedGroupId;
      shareStatus = result.existing
        ? `${selectedRoom} already has this highlight.`
        : `Shared to ${selectedRoom}.`;
      selectedGroupId = '';
    } catch (error) {
      shareError = error instanceof Error ? error.message : 'Could not share the highlight.';
    } finally {
      sharing = false;
    }
  }

  function shortPubkey(value: string): string {
    if (!value) return '';
    return `${value.slice(0, 8)}…${value.slice(-4)}`;
  }

  function buildExcerptSegments(context: string, quote: string): Array<{ text: string; marked: boolean }> {
    const normalizedContext = compactWhitespace(context);
    const normalizedQuote = compactWhitespace(quote);

    if (!normalizedContext && !normalizedQuote) {
      return [];
    }

    if (!normalizedContext || normalizedContext.toLocaleLowerCase() === normalizedQuote.toLocaleLowerCase()) {
      return [{ text: normalizedQuote || normalizedContext, marked: true }];
    }

    const matchIndex = normalizedContext.toLocaleLowerCase().indexOf(normalizedQuote.toLocaleLowerCase());
    if (normalizedQuote && matchIndex >= 0) {
      return [
        { text: normalizedContext.slice(0, matchIndex), marked: false },
        {
          text: normalizedContext.slice(matchIndex, matchIndex + normalizedQuote.length),
          marked: true
        },
        {
          text: normalizedContext.slice(matchIndex + normalizedQuote.length),
          marked: false
        }
      ].filter((segment) => segment.text.length > 0);
    }

    if (!normalizedQuote) {
      return [{ text: normalizedContext, marked: false }];
    }

    return [
      { text: normalizedContext, marked: false },
      { text: ' … ', marked: false },
      { text: normalizedQuote, marked: true }
    ];
  }

  function compactWhitespace(value: string): string {
    return value.trim().replace(/\s+/g, ' ');
  }

  function handleSeek() {
    if (seekTo && highlight.clipStartSeconds != null) {
      seekTo(highlight.clipStartSeconds);
    }
  }
</script>

<article class="grid gap-3.5 border-b border-base-300/50 pb-4 last:border-b-0 last:pb-0">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="flex flex-wrap items-baseline gap-2">
      <User.Root {ndk} pubkey={highlight.pubkey}>
        <a class="text-sm font-bold text-base-content no-underline hover:text-primary" href={`/profile/${highlight.pubkey}`}>
          <User.Name fallback={shortPubkey(highlight.pubkey)} />
        </a>
      </User.Root>

      {#if createdLabel}
        <span class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-base-content/60">
          {createdLabel}
        </span>
      {/if}

      {#if clipLabel}
        <button
          type="button"
          class="inline-flex min-h-7 cursor-pointer items-center rounded-full border-0 bg-base-200 px-2.5 text-xs font-semibold text-base-content/60 disabled:cursor-default"
          onclick={handleSeek}
          disabled={!canSeek}
        >
          {clipLabel}
        </button>
      {/if}
    </div>

    {#if primaryShare}
      <a
        class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-primary no-underline"
        href={highlightPath(primaryShare.groupId, highlight.eventId)}
      >
        Public card
      </a>
    {/if}
  </div>

  <blockquote class="m-0 border-l-2 border-primary pl-4">
    <p class="m-0 font-serif text-lg leading-snug text-base-content [&_mark]:rounded-sm [&_mark]:bg-primary/15 [&_mark]:px-0.5 [&_mark]:text-inherit">
      {#each excerptSegments as segment (segment.text)}
        {#if segment.marked}
          <mark>{segment.text}</mark>
        {:else}
          {segment.text}
        {/if}
      {/each}
    </p>
  </blockquote>

  {#if highlight.note}
    <p class="m-0 leading-relaxed text-base-content">{highlight.note}</p>
  {/if}

  {#if highlight.clipSpeaker}
    <p class="m-0 text-sm font-semibold leading-relaxed text-base-content/60">Speaker: {highlight.clipSpeaker}</p>
  {/if}

  <div class="flex flex-wrap items-start justify-between gap-2 max-sm:flex-col max-sm:items-stretch">
    <div class="flex flex-wrap items-center gap-2">
      {#if artifactPageHref}
        <a class="text-sm font-bold text-base-content no-underline hover:text-primary" href={artifactPageHref}>{sourceTitle}</a>
      {:else if sourceHref}
        <a class="text-sm font-bold text-base-content no-underline hover:text-primary" href={sourceHref} target="_blank" rel="noreferrer">{sourceTitle}</a>
      {:else}
        <span class="pointer-events-none text-sm font-bold text-base-content">{sourceTitle}</span>
      {/if}

      {#each sourceMeta as item (item)}
        <span class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-base-content/60">{item}</span>
      {/each}

      {#if highlight.shareCount > 0}
        <span class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-base-content/60">
          {highlight.shareCount} room{highlight.shareCount === 1 ? '' : 's'}
        </span>
      {/if}
    </div>

    {#if sourceHref}
      <a
        class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-base-content/60 no-underline"
        href={sourceHref}
        target="_blank"
        rel="noreferrer"
      >Open source</a>
    {/if}
  </div>

  {#if canDiscuss}
    <div class="flex gap-1.5">
      <button
        type="button"
        class="inline-flex min-h-7 cursor-pointer items-center rounded-full border-0 bg-base-200 px-2.5 text-xs font-semibold text-base-content/60 hover:text-primary"
        class:!text-primary={showThread}
        onclick={() => (showThread = !showThread)}
      >
        {showThread ? 'Hide discussion' : 'Discuss'}
      </button>
    </div>

    {#if showThread}
      <div class="border-t border-base-300 pt-3.5">
        <DiscussionPanel
          {groupId}
          rootContext={{ type: 'highlight', highlightEventId: highlight.eventId }}
          compact
          maxVisible={3}
        />
      </div>
    {/if}
  {/if}

  {#if showShareControl && rooms.length > 0}
    <div class="flex flex-wrap items-center gap-2 max-sm:flex-col max-sm:items-stretch">
      <select
        class="select select-bordered max-sm:w-full"
        bind:value={selectedGroupId}
        disabled={shareableRooms.length === 0 || sharing}
      >
        {#if shareableRooms.length === 0}
          <option value="">Already shared everywhere loaded here</option>
        {:else}
          {#each shareableRooms as room (room.id)}
            <option value={room.id}>{room.name}</option>
          {/each}
        {/if}
      </select>

      <button
        type="button"
        class="btn btn-primary rounded-full max-sm:w-full"
        disabled={!canShareAgain}
        onclick={handleShareAgain}
      >
        {sharing ? 'Sharing…' : 'Share to room'}
      </button>
    </div>

    {#if shareError}
      <p class="m-0 text-sm leading-relaxed text-error">{shareError}</p>
    {/if}

    {#if shareStatus}
      <p class="m-0 text-sm leading-relaxed text-success">{shareStatus}</p>
    {/if}
  {/if}
</article>
