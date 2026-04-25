<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import RoomHeader from '$lib/features/room/components/RoomHeader.svelte';
  import Block from '$lib/features/room/components/Block.svelte';
  import * as Tabs from '$lib/components/ui/tabs';
  import ArtifactForm from '$lib/features/artifacts/ArtifactForm.svelte';
  import PinnedArtifact from '$lib/features/room/components/PinnedArtifact.svelte';
  import AlsoCard from '$lib/features/room/components/AlsoCard.svelte';
  import ShelfTile from '$lib/features/room/components/ShelfTile.svelte';
  import SeeAllLink from '$lib/features/room/components/SeeAllLink.svelte';
  import HighlightCard from '$lib/features/room/components/HighlightCard.svelte';
  import MembersSidebar from '$lib/features/room/components/MembersSidebar.svelte';
  import UpNextVoting from '$lib/features/room/components/UpNextVoting.svelte';
  import CaptureCta from '$lib/features/room/components/CaptureCta.svelte';
  import DiscussionRow from '$lib/features/room/components/DiscussionRow.svelte';
  import ChatTab from '$lib/features/room/components/ChatTab.svelte';
  import {
    discussionFromEvent,
    discussionPath,
    isDiscussionThread
  } from '$lib/features/discussions/roomDiscussion';
  import { HIGHLIGHTER_COMMENT_KIND } from '$lib/features/discussions/discussion';
  import { relativeTime } from '$lib/utils/time';
  import {
    KIND_PIN,
    artifactFromThreadEvent,
    type RoomMember,
    type Highlight,
    type Artifact
  } from '$lib/features/room/api/types';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const roomTitle = $derived(data.room?.name ?? data.room?.id ?? '');
  const members = $derived<RoomMember[]>(data.room?.members ?? []);
  const slug = $derived(data.room?.id);
  const currentUser = $derived(ndk.$currentUser);
  const isMember = $derived(
    !!currentUser && members.some((m) => m.pubkey === currentUser.pubkey)
  );
  const isAdmin = $derived(
    !!currentUser && (data.room?.adminPubkeys ?? []).includes(currentUser.pubkey)
  );

  // Client-side subscriptions for NIP-29 room content, scoped by `#h` tag.
  // SSR currently ships only metadata + members; this hydrates the shelf,
  // highlights reel, and pinned artifact from relay.highlighter.com.
  const threadsFeed = ndk.$subscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [{ kinds: [NDKKind.Thread], '#h': [slug], limit: 32 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const highlightsFeed = ndk.$metaSubscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [
        { kinds: [NDKKind.Highlight], '#h': [slug], limit: 64 },
        { kinds: [16 as number], '#h': [slug], '#k': ["9802"], limit: 64 }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const pinsFeed = ndk.$subscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [{ kinds: [KIND_PIN], '#h': [slug], limit: 10 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const sortedThreads = $derived(
    [...threadsFeed.events].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0))
  );

  const artifacts = $derived<Artifact[]>(
    sortedThreads.filter((e) => !isDiscussionThread(e)).map(artifactFromThreadEvent)
  );

  const discussions = $derived(
    sortedThreads.filter((e) => isDiscussionThread(e)).map(discussionFromEvent)
  );

  const discussionRepliesFeed = ndk.$subscribe(() => {
    if (!browser || !slug || discussions.length === 0) return undefined;
    return {
      filters: [
        {
          kinds: [HIGHLIGHTER_COMMENT_KIND],
          '#E': discussions.map((d) => d.eventId),
          '#h': [slug],
          limit: 200
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const replyStatsByThread = $derived.by(() => {
    const stats = new Map<string, { count: number; lastAt: number; pubkeys: Set<string> }>();
    for (const ev of discussionRepliesFeed.events) {
      const rootId = ev.getMatchingTags('E')[0]?.[1];
      if (!rootId) continue;
      const entry = stats.get(rootId) ?? { count: 0, lastAt: 0, pubkeys: new Set<string>() };
      entry.count += 1;
      entry.lastAt = Math.max(entry.lastAt, ev.created_at ?? 0);
      entry.pubkeys.add(ev.pubkey);
      stats.set(rootId, entry);
    }
    return stats;
  });

  const discussionRows = $derived(
    discussions.map((d) => {
      const stats = replyStatsByThread.get(d.eventId);
      const replyCount = stats?.count ?? 0;
      const lastTimestamp = stats && stats.lastAt > 0 ? stats.lastAt : d.createdAt;
      const participantPubkeys = new Set<string>([d.pubkey, ...(stats?.pubkeys ?? [])]);
      const participants = [...participantPubkeys].slice(0, 4).map((pubkey) => ({
        pubkey,
        colorIndex: colorByPubkey.get(pubkey) ?? 1
      }));
      return {
        id: d.id,
        eventId: d.eventId,
        title: d.title,
        starterPubkey: d.pubkey,
        participants,
        replyCount,
        lastAt: relativeTime(lastTimestamp),
        href: discussionPath(slug ?? '', d.id),
        status: (Date.now() / 1000 - lastTimestamp < 60 * 60 * 24 * 7 ? 'active' : 'closed') as
          | 'active'
          | 'closed'
      };
    })
  );

  const colorByPubkey = $derived(new Map(members.map((m) => [m.pubkey, m.colorIndex])));

  const highlights = $derived.by<Highlight[]>(() => {
    const events = [...highlightsFeed.events];
    const highlightMap = new Map<string, any>();
    const timestampMap = new Map<string, number>();

    for (const event of events) {
      if (event.kind === NDKKind.Highlight) {
        highlightMap.set(event.id, event);
        if (event.tagValue('h') === slug) {
          const current = timestampMap.get(event.id) || 0;
          timestampMap.set(event.id, Math.max(current, event.created_at ?? 0));
        }
      } else if (event.kind === 16) {
        const originalId = event.tagValue('e');
        if (originalId) {
          const current = timestampMap.get(originalId) || 0;
          timestampMap.set(originalId, Math.max(current, event.created_at ?? 0));
        }
      }
    }

    return Array.from(timestampMap.entries())
      .map(([id, timestamp]) => {
        const event = highlightMap.get(id);
        if (!event) return null;
        return {
          id: event.id,
          artifactId: event.tagValue('a') || event.tagValue('e') || '',
          quote: event.content.trim(),
          authorPubkey: event.pubkey,
          authorColorIndex: colorByPubkey.get(event.pubkey) ?? 1,
          createdAt: timestamp
        };
      })
      .filter((h): h is Highlight => h !== null)
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, 30);
  });

  const pinnedArtifact = $derived.by<Artifact | undefined>(() => {
    const latestPin = [...pinsFeed.events].sort(
      (a, b) => (b.created_at ?? 0) - (a.created_at ?? 0)
    )[0];
    if (latestPin) {
      const pinnedThreadId = latestPin.tagValue('e');
      if (pinnedThreadId) {
        const match = artifacts.find((a) => a.id === pinnedThreadId);
        if (match) return match;
      }
    }
    return artifacts[0];
  });

  const sections = $derived([
    { id: 'pinned', label: 'Pinned' },
    { id: 'this-week', label: 'This week' },
    { id: 'shelf', label: 'Library', count: artifacts.length },
    { id: 'highlights', label: 'Highlights', count: highlights.length },
    { id: 'discussions', label: 'Discussions', count: discussions.length },
    { id: 'chat', label: 'Chat' },
    { id: 'lately', label: 'Activity' }
  ]);

  let activeTab = $state('pinned');
  let castDialogOpen = $state(false);

  // Highlights filtered to the pinned artifact — for the pinned card Highlights tab
  const pinnedHighlights = $derived(
    pinnedArtifact
      ? highlights
          .filter((h) => h.artifactId === pinnedArtifact.id)
          .slice(0, 10)
          .map((h) => ({
            id: h.id,
            authorPubkey: h.authorPubkey,
            colorIndex: h.authorColorIndex,
            quote: h.quote,
            date: relativeTime(h.createdAt)
          }))
      : []
  );

  // Per-member filter counts for the pinned Highlights tab
  const pinnedMemberFilters = $derived.by(() => {
    if (!pinnedArtifact) return [];
    const counts = new Map<string, { colorIndex: number; count: number }>();
    for (const h of highlights) {
      if (h.artifactId !== pinnedArtifact.id) continue;
      const entry = counts.get(h.authorPubkey);
      if (entry) entry.count++;
      else counts.set(h.authorPubkey, { colorIndex: h.authorColorIndex, count: 1 });
    }
    return [...counts.entries()].map(([pubkey, { colorIndex, count }]) => ({
      pubkey,
      colorIndex,
      count
    }));
  });

  // Members tab rows — just pubkeys + position colors, real data fetched by component
  const membersTableRows = $derived(
    members.map((m) => ({
      pubkey: m.pubkey,
      colorIndex: m.colorIndex
    }))
  );

  const thisWeek = $derived(artifacts.slice(0, 2));
  const shelfItems = $derived(artifacts);

  const highlightReel = $derived(
    highlights.slice(0, 9).map((h) => {
      const art = artifacts.find((a) => a.id === h.artifactId);
      return {
        id: h.id,
        quote: h.quote,
        sourceTitle: art ? art.title : '',
        sourceSub: relativeTime(h.createdAt),
        marks: [{ pubkey: h.authorPubkey, colorIndex: h.authorColorIndex }],
        date: relativeTime(h.createdAt),
        href: art ? `/r/${data.room?.id}/e/${art.id}` : '#'
      };
    })
  );

  function pinnedCoverVariant(type: Artifact['type']): 'dark' | 'red' | 'blue' | 'green' | 'plum' {
    switch (type) {
      case 'book': return 'dark';
      case 'podcast': return 'plum';
      case 'essay': return 'red';
      case 'article': return 'blue';
      case 'video': return 'green';
      default: return 'dark';
    }
  }

  function artifactHref(id: string): string {
    return `/r/${data.room?.id}/e/${id}`;
  }

  function alsoType(
    type: Artifact['type']
  ): 'book' | 'podcast' | 'essay' | 'article' | 'paper' {
    return type === 'video' ? 'article' : type;
  }

  function shelfType(type: Artifact['type']): 'book' | 'podcast' | 'essay' | 'paper' | 'archive' {
    if (type === 'book' || type === 'podcast' || type === 'essay') return type;
    if (type === 'article') return 'essay';
    return 'archive';
  }
</script>

<svelte:head>
  <title>{roomTitle ? `${roomTitle} · Room` : 'Room'}</title>
</svelte:head>

{#if !data.room}
  <div class="flex flex-col items-center text-center py-20 gap-4">
    <h1 class="font-serif text-4xl font-normal text-base-content m-0">Room not found</h1>
    <p class="text-base-content/80 text-[15px] max-w-[44ch] m-0">No room was found at this address, or the relay doesn't hold its metadata yet.</p>
    <a href="/rooms" class="btn">Back to your rooms</a>
  </div>
{:else}
  <RoomHeader title={roomTitle} {members} />

  <Tabs.Root bind:value={activeTab}>
    <div class="roomtabs-bar">
      <Tabs.List>
        {#each sections as section (section.id)}
          <Tabs.Trigger value={section.id}>
            {section.label}
            {#if section.count !== undefined}
              <span class="font-mono text-[10.5px] opacity-50">{section.count}</span>
            {/if}
          </Tabs.Trigger>
        {/each}
      </Tabs.List>
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)_380px] gap-11 py-11 pb-20 max-[1060px]:grid-cols-1 max-[1060px]:gap-8">
      <div class="min-w-0">
        <Tabs.Content value="pinned">
          <Block id="pinned" title="Currently pinned." accent="pinned.">
            {#if pinnedArtifact}
              <PinnedArtifact
                title={pinnedArtifact.title}
                subtitle={pinnedArtifact.author}
                coverTitle={pinnedArtifact.title}
                coverAuthor={pinnedArtifact.author}
                coverVariant={pinnedCoverVariant(pinnedArtifact.type)}
                image={pinnedArtifact.cover || undefined}
                openHref={pinnedArtifact.url || '#'}
                continueHref={artifactHref(pinnedArtifact.id)}
                continueLabel={pinnedArtifact.type === 'podcast' ? 'Continue listening' : 'Continue reading'}
                stats={[
                  { value: String(members.length), label: 'members' },
                  { value: String(pinnedHighlights.length), label: 'highlights' }
                ]}
                readers={[]}
                tabCounts={{
                  discussions: 0,
                  highlights: pinnedHighlights.length,
                  notes: 0,
                  members: members.length
                }}
                passageSpans={[]}
                messages={[]}
                highlights={pinnedHighlights}
                memberFilters={pinnedMemberFilters}
                notes={[]}
                membersTableRows={membersTableRows}
                defaultTab="Highlights"
              />
            {:else}
              <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic">
                <p>No artifact has been pinned yet. Share the first read.</p>
              </div>
            {/if}
          </Block>
        </Tabs.Content>

        <Tabs.Content value="this-week">
          <Block id="this-week" title="Shared this week." accent="this week.">
            {#if thisWeek.length === 0}
              <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic"><p>Nothing else shared this week.</p></div>
            {:else}
              <div class="grid grid-cols-2 gap-4 max-[760px]:grid-cols-1">
                {#each thisWeek as art (art.id)}
                  <AlsoCard
                    href={artifactHref(art.id)}
                    type={alsoType(art.type)}
                    sharedBy=""
                    when=""
                    artworkLabel={art.title.slice(0, 4).toUpperCase()}
                    title={art.title}
                    source={art.author}
                    engaged={[]}
                  />
                {/each}
              </div>
            {/if}
          </Block>
        </Tabs.Content>

        <Tabs.Content value="shelf">
          <Block id="shelf" title="Your library." accent="library.">
            {#if shelfItems.length === 0}
              <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic"><p>The library is empty. Share something to read.</p></div>
            {:else}
              <div class="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-3.5">
                {#each shelfItems as art (art.id)}
                  <ShelfTile
                    id={art.id}
                    href={artifactHref(art.id)}
                    type={shelfType(art.type)}
                    typeChipLabel={art.type}
                    title={art.title}
                    author={art.author}
                    engaged={[]}
                    stats={`${art.highlightCount} hl`}
                  />
                {/each}
              </div>
              {#if shelfItems.length >= 12}
                <SeeAllLink label="See all {shelfItems.length} in the library" href="#" />
              {/if}
            {/if}
          </Block>
        </Tabs.Content>

        <Tabs.Content value="highlights">
          <Block id="highlights" title="The room's highlights." accent="highlights.">
            {#if highlightReel.length === 0}
              <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic"><p>No highlights yet. Be the first.</p></div>
            {:else}
              <div class="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3.5">
                {#each highlightReel as hl (hl.id)}
                  <HighlightCard
                    id={hl.id}
                    quote={hl.quote}
                    sourceTitle={hl.sourceTitle}
                    sourceSub={hl.sourceSub}
                    marks={hl.marks}
                    date={hl.date}
                    href={hl.href}
                  />
                {/each}
              </div>
              {#if highlights.length > highlightReel.length}
                <SeeAllLink label="See all {highlights.length} highlights" href="#" />
              {/if}
            {/if}
          </Block>
        </Tabs.Content>

        <Tabs.Content value="discussions">
          <Block id="discussions" title="Discussions">
            {#snippet filters()}
              {#if isMember}
                <div class="flex justify-end ml-auto">
                  <a class="btn btn-primary btn-sm" href="/r/{slug}/discussions/new">
                    New discussion
                  </a>
                </div>
              {/if}
            {/snippet}

            {#if discussionRows.length === 0}
              <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic">
                <p>
                  {#if isMember}
                    No discussions yet.
                  {:else}
                    No discussions yet. Join the room to start one.
                  {/if}
                </p>
              </div>
            {:else}
              <div class="bg-base-100 border border-base-300 rounded overflow-hidden mt-4">
                {#each discussionRows as row (row.eventId)}
                  <DiscussionRow
                    id={row.id}
                    status={row.status}
                    title={row.title}
                    starterPubkey={row.starterPubkey}
                    participants={row.participants}
                    replies={row.replyCount}
                    lastAt={row.lastAt}
                    href={row.href}
                  />
                {/each}
              </div>
            {/if}
          </Block>
        </Tabs.Content>

        <Tabs.Content value="chat">
          {#if slug}
            <ChatTab groupId={slug} {isMember} />
          {/if}
        </Tabs.Content>

        <Tabs.Content value="lately">
          <Block id="lately" title="Recent activity." accent="activity.">
            <div class="py-9 px-7 text-center bg-base-100 border border-dashed border-base-300 rounded text-base-content/50 text-sm [&_p]:m-0 [&_p]:italic">
              <p>Nothing has happened yet.</p>
            </div>
          </Block>
        </Tabs.Content>
      </div>

      <aside class="flex flex-col gap-6 max-[1060px]:flex-none min-[1060px]:sticky min-[1060px]:top-[112px] min-[1060px]:self-start min-[1060px]:max-h-[calc(100vh-140px)] min-[1060px]:overflow-y-auto">
        {#if members.length > 0}
          <MembersSidebar members={members.map((m) => ({ pubkey: m.pubkey, colorIndex: m.colorIndex }))} slug={data.room?.id ?? ''} {isAdmin} />
        {/if}
        <UpNextVoting
          items={[]}
          closesText="Nothing suggested yet."
          showSuggest={isMember}
          onSuggest={() => (castDialogOpen = true)}
        />
        {#if isMember}<CaptureCta />{/if}
      </aside>
    </div>
  </Tabs.Root>

  {#if slug}
    <ArtifactForm groupId={slug} bind:open={castDialogOpen} />
  {/if}
{/if}

<style>
  .roomtabs-bar {
    position: sticky;
    top: 62px;
    background: var(--bg);
    z-index: 15;
    margin: 0 calc(var(--container-px) * -1);
    padding: 0 var(--container-px);
    overflow-x: auto;
  }
</style>
