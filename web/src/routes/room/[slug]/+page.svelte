<script lang="ts">
  import RoomHeader from '$lib/features/room/components/RoomHeader.svelte';
  import RoomNav from '$lib/features/room/components/RoomNav.svelte';
  import Block from '$lib/features/room/components/Block.svelte';
  import PinnedArtifact from '$lib/features/room/components/PinnedArtifact.svelte';
  import AlsoCard from '$lib/features/room/components/AlsoCard.svelte';
  import ShelfTile from '$lib/features/room/components/ShelfTile.svelte';
  import SeeAllLink from '$lib/features/room/components/SeeAllLink.svelte';
  import HighlightCard from '$lib/features/room/components/HighlightCard.svelte';
  import MembersSidebar from '$lib/features/room/components/MembersSidebar.svelte';
  import UpNextVoting from '$lib/features/room/components/UpNextVoting.svelte';
  import CaptureCta from '$lib/features/room/components/CaptureCta.svelte';
  import { relativeTime } from '$lib/utils/time';
  import type { PageData } from './$types';
  import type { RoomMember, Highlight, Artifact } from '$lib/features/room/api/room';

  let { data }: { data: PageData } = $props();

  const roomTitle = $derived(data.room?.name ?? data.room?.id ?? '');
  const members = $derived<RoomMember[]>(data.room?.members ?? []);
  const artifacts = $derived<Artifact[]>(data.room?.artifacts ?? []);
  const highlights = $derived<Highlight[]>(data.room?.highlights ?? []);
  const pinnedArtifact = $derived<Artifact | undefined>(data.room?.pinnedArtifact);

  const sections = $derived([
    { id: 'pinned', label: 'Pinned' },
    { id: 'this-week', label: 'This week' },
    { id: 'shelf', label: 'The shelf', count: artifacts.length },
    { id: 'highlights', label: 'Highlights', count: highlights.length },
    { id: 'discussions', label: 'Discussions' },
    { id: 'lately', label: 'Lately' }
  ]);

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
        href: art ? `/room/${data.room?.id}/artifact/${art.id}` : '#'
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
    return `/room/${data.room?.id}/artifact/${id}`;
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
  <div class="room-missing">
    <h1>Room not found</h1>
    <p>No room was found at this address, or the relay doesn't hold its metadata yet.</p>
    <a href="/rooms" class="btn">Back to your rooms</a>
  </div>
{:else}
  <RoomHeader title={roomTitle} {members} />
  <RoomNav {sections} />

  <div class="room-main">
    <div class="room-content">
      <Block id="pinned" title="Currently pinned." accent="pinned.">
        {#if pinnedArtifact}
          <PinnedArtifact
            title={pinnedArtifact.title}
            subtitle={pinnedArtifact.author}
            coverTitle={pinnedArtifact.title}
            coverAuthor={pinnedArtifact.author}
            coverVariant={pinnedCoverVariant(pinnedArtifact.type)}
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
          <div class="empty-card">
            <p>No artifact has been pinned yet. Share the first read.</p>
          </div>
        {/if}
      </Block>

      <Block id="this-week" title="Also this week." accent="week.">
        {#if thisWeek.length === 0}
          <div class="empty-card"><p>Nothing else shared this week.</p></div>
        {:else}
          <div class="also-grid">
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

      <Block id="shelf" title="The shelf." accent="shelf.">
        {#if shelfItems.length === 0}
          <div class="empty-card"><p>The shelf is empty. Share something to read.</p></div>
        {:else}
          <div class="shelf-grid">
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
            <SeeAllLink label="See all {shelfItems.length} on the shelf" href="#" />
          {/if}
        {/if}
      </Block>

      <Block id="highlights" title="The room's highlights." accent="highlights.">
        {#if highlightReel.length === 0}
          <div class="empty-card"><p>No highlights yet. Be the first.</p></div>
        {:else}
          <div class="hl-reel">
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

      <Block id="discussions" title="Every discussion." accent="discussion.">
        <div class="empty-card">
          <p>No discussions yet. Start one on a highlighted passage.</p>
        </div>
      </Block>

      <Block id="lately" title="Lately in the room." accent="room.">
        <div class="empty-card">
          <p>Nothing has happened yet.</p>
        </div>
      </Block>
    </div>

    <aside class="sidebar">
      {#if members.length > 0}
        <MembersSidebar members={members.map((m) => ({ pubkey: m.pubkey, colorIndex: m.colorIndex }))} />
      {/if}
      <UpNextVoting items={[]} closesText="Nothing proposed yet." />
      <CaptureCta />
    </aside>
  </div>
{/if}

<style>
  .room-missing {
    padding: 80px 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: center;
  }

  .room-missing h1 {
    font-family: var(--font-serif);
    font-size: 36px;
    font-weight: 400;
    color: var(--ink);
    margin: 0;
  }

  .room-missing p {
    color: var(--ink-soft);
    font-size: 15px;
    max-width: 44ch;
    margin: 0;
  }

  .btn {
    padding: 10px 20px;
    background: var(--ink);
    color: var(--surface);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: var(--radius);
    transition: background 200ms ease;
  }

  .btn:hover { background: var(--brand-accent); }

  .room-main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--grid-sidebar);
    gap: var(--grid-gap);
    padding: 44px 0 80px;
  }

  @media (max-width: 1060px) {
    .room-main { grid-template-columns: 1fr; gap: 32px; }
  }

  .room-content { min-width: 0; }

  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  @media (min-width: 1060px) {
    .sidebar {
      position: sticky;
      top: 112px;
      align-self: start;
      max-height: calc(100vh - 140px);
      overflow-y: auto;
    }
  }

  .also-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  @media (max-width: 760px) {
    .also-grid { grid-template-columns: 1fr; }
  }

  .shelf-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 14px;
  }

  .hl-reel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 14px;
  }

  .empty-card {
    padding: 36px 28px;
    text-align: center;
    background: var(--surface);
    border: 1px dashed var(--rule);
    border-radius: var(--radius);
    color: var(--ink-fade);
    font-family: var(--font-sans);
    font-size: 14px;
  }

  .empty-card p {
    margin: 0;
    font-style: italic;
  }
</style>
