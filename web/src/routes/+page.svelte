<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import HighlightSourceGroup from '$lib/features/highlights/HighlightSourceGroup.svelte';
  import { groupHighlightsBySource } from '$lib/features/highlights/grouping';
  import { fetchArtifactsByHighlightReferenceKeys } from '$lib/ndk/artifacts';
  import { ndk } from '$lib/ndk/client';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    HIGHLIGHTER_HIGHLIGHT_KIND,
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    fetchHighlightsForShares,
    hydrateStandaloneHighlights
  } from '$lib/ndk/highlights';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';
  import {
    guestActions,
    memberActions,
    type SurfaceAction
  } from '$lib/highlighter/surfaces';
  import type { HydratedHighlight } from '$lib/ndk/highlights';
  import TopNav from '$lib/features/room/components/TopNav.svelte';
  import Footer from '$lib/features/room/components/Footer.svelte';

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));
  const actions = $derived((signedIn ? memberActions : guestActions) as SurfaceAction[]);

  /* ── Room membership ── */
  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();
    for (const event of membershipFeed.events) {
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }
    return [...ids];
  });

  const metadataFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupMetadata], '#d': membershipGroupIds, limit: Math.max(membershipGroupIds.length * 2, 32) }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const rooms = $derived(
    currentUser
      ? buildJoinedRooms(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
      : []
  );

  /* ── Room highlight shares ── */
  const roomShareFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': membershipGroupIds, limit: 256 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  let roomHighlights = $state<HydratedHighlight[]>([]);
  let fetchingRoomHighlights = $state(false);

  $effect(() => {
    const shareEvents = [...roomShareFeed.events];
    if (!browser || shareEvents.length === 0) {
      roomHighlights = [];
      fetchingRoomHighlights = false;
      return;
    }

    let cancelled = false;
    fetchingRoomHighlights = true;

    void fetchHighlightsForShares(ndk, shareEvents)
      .then((highlights) => {
        if (!cancelled) roomHighlights = highlights;
      })
      .finally(() => {
        if (!cancelled) fetchingRoomHighlights = false;
      });

    return () => { cancelled = true; };
  });

  /* ── Follow highlights ── */
  const followPubkeys = $derived.by(() => {
    const all = [...(ndk.$follows ?? [])];
    return all.slice(0, 500);
  });

  const followHighlightFeed = ndk.$subscribe(() => {
    if (!browser || followPubkeys.length === 0) return undefined;
    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_KIND], authors: followPubkeys, limit: 50 }],
      relayUrls: DEFAULT_RELAYS,
      closeOnEose: true
    };
  });

  const followHighlights = $derived(
    hydrateStandaloneHighlights([...followHighlightFeed.events])
  );

  /* ── Merge + deduplicate ── */
  const mergedHighlights = $derived.by(() => {
    const byEventId = new Map<string, HydratedHighlight>();
    for (const hl of roomHighlights) {
      byEventId.set(hl.eventId, hl);
    }
    for (const hl of followHighlights) {
      if (!byEventId.has(hl.eventId)) {
        byEventId.set(hl.eventId, hl);
      }
    }
    return [...byEventId.values()].toSorted(
      (a, b) => (b.latestSharedAt ?? b.createdAt ?? 0) - (a.latestSharedAt ?? a.createdAt ?? 0)
    );
  });

  /* ── Artifact resolution ── */
  let artifactsByReference = $state<Map<string, ArtifactRecord>>(new Map());
  let resolvingArtifacts = $state(false);

  $effect(() => {
    if (!browser) {
      artifactsByReference = new Map();
      return;
    }
    const referenceKeys = [...new Set(mergedHighlights.map((hl) => hl.sourceReferenceKey).filter(Boolean))];
    if (referenceKeys.length === 0) {
      artifactsByReference = new Map();
      return;
    }
    let cancelled = false;
    resolvingArtifacts = true;
    void fetchArtifactsByHighlightReferenceKeys(ndk, referenceKeys)
      .then((artifacts) => { if (!cancelled) artifactsByReference = artifacts; })
      .finally(() => { if (!cancelled) resolvingArtifacts = false; });
    return () => { cancelled = true; };
  });

  const feedGroups = $derived(groupHighlightsBySource(mergedHighlights, artifactsByReference));

  /* ── Feed pagination ── */
  let showAll = $state(false);
  const INITIAL_GROUP_COUNT = 8;
  const visibleGroups = $derived(showAll ? feedGroups : feedGroups.slice(0, INITIAL_GROUP_COUNT));

  /* ── State helpers ── */
  const isLoading = $derived(!membershipFeed.eosed || fetchingRoomHighlights);
  const hasRooms = $derived(membershipGroupIds.length > 0);
  const hasFollows = $derived(followPubkeys.length > 0);
  const isEmpty = $derived(!isLoading && mergedHighlights.length === 0);

</script>

{#if signedIn}
  <!-- ═══ SIGNED-IN FEED ═══ -->
  <section class="flex items-end justify-between gap-4 pt-6 max-sm:flex-col max-sm:items-start">
    <div>
      <h1 class="m-0 text-base-content font-serif text-[clamp(1.5rem,3.5vw,2rem)] leading-[1.15] tracking-[-0.02em]">Your feed</h1>
    </div>
    <div class="flex flex-wrap gap-[0.625rem] shrink-0">
      {#each memberActions as action (action.href)}
        <a
          href={action.href}
          class={action.tone === 'secondary' ? 'btn btn-ghost btn-sm' : 'btn btn-primary btn-sm'}
        >
          {action.label}
        </a>
      {/each}
    </div>
  </section>

  <section class="grid grid-cols-[1fr_22rem] gap-8 items-start max-[820px]:grid-cols-1">
    <div class="min-w-0">
      {#if isLoading}
        <div class="grid gap-[0.9rem]">
          <div class="skeleton-card"></div>
          <div class="skeleton-card"></div>
          <div class="skeleton-card"></div>
        </div>
        <p class="m-0 text-base-content/50 text-[0.88rem]">Loading your feed...</p>
      {:else if isEmpty && !hasRooms && !hasFollows}
        <div class="grid gap-2 px-8 py-10 border border-base-300 rounded-2xl bg-base-100 text-center">
          <h2 class="m-0 text-base-content font-serif text-[1.35rem]">Your feed starts here.</h2>
          <p class="m-0 text-base-content/50 text-[0.95rem] leading-relaxed">Join a room or follow someone to see highlights appear in your feed.</p>
          <div class="flex justify-center gap-3 mt-4">
            <a href="/discover" class="btn btn-primary btn-sm">Discover rooms</a>
            <a href="/r/create" class="btn btn-ghost btn-sm">Create a room</a>
          </div>
        </div>
      {:else if isEmpty && hasRooms}
        <div class="grid gap-2 px-8 py-10 border border-base-300 rounded-2xl bg-base-100 text-center">
          <h2 class="m-0 text-base-content font-serif text-[1.35rem]">Your rooms are quiet.</h2>
          <p class="m-0 text-base-content/50 text-[0.95rem] leading-relaxed">No highlights have been shared yet. Be the first.</p>
          <div class="flex justify-center gap-3 mt-4">
            <a href="/rooms" class="btn btn-primary btn-sm">Visit your rooms</a>
          </div>
        </div>
      {:else if isEmpty && hasFollows}
        <div class="grid gap-2 px-8 py-10 border border-base-300 rounded-2xl bg-base-100 text-center">
          <h2 class="m-0 text-base-content font-serif text-[1.35rem]">Nothing new from your network.</h2>
          <p class="m-0 text-base-content/50 text-[0.95rem] leading-relaxed">The people you follow haven't shared highlights recently.</p>
          <div class="flex justify-center gap-3 mt-4">
            <a href="/discover" class="btn btn-primary btn-sm">Discover rooms</a>
          </div>
        </div>
      {:else}
        <div class="grid gap-[0.9rem]">
          {#each visibleGroups as group (group.referenceKey)}
            <HighlightSourceGroup {group} {rooms} showShareControl={true} />
          {/each}
        </div>
        {#if !showAll && feedGroups.length > INITIAL_GROUP_COUNT}
          <button class="btn btn-ghost mt-4 w-full text-center border-none" onclick={() => showAll = true}>
            Show more ({feedGroups.length - INITIAL_GROUP_COUNT} remaining)
          </button>
        {/if}
        {#if resolvingArtifacts}
          <p class="mt-3 m-0 text-base-content/50 text-[0.88rem]">Resolving source details...</p>
        {/if}
      {/if}
    </div>

    <aside class="grid gap-5 max-[820px]:order-1">
      {#if rooms.length > 0}
        <div class="grid gap-2">
          <h3 class="m-0 text-[0.75rem] font-medium tracking-[0.08em] uppercase text-base-content/50">Your rooms</h3>
          <ul class="list-none m-0 p-0 grid gap-0.5">
            {#each rooms.slice(0, 6) as room (room.id)}
              <li>
                <a href="/r/{room.id}" class="block py-[0.45rem] px-[0.625rem] rounded-lg text-base-content text-[0.9rem] no-underline transition-[background] duration-[120ms] ease hover:bg-base-200">{room.name}</a>
              </li>
            {/each}
          </ul>
          {#if rooms.length > 6}
            <a href="/rooms" class="text-[0.813rem] text-primary no-underline pl-[0.625rem] hover:underline">View all rooms</a>
          {/if}
        </div>
      {/if}
      <div class="grid gap-2 pt-5 border-t border-base-300">
        <h3 class="m-0 text-base text-base-content font-semibold">Start a new room</h3>
        <p class="m-0 text-[0.875rem] text-base-content/50 leading-[1.5]">Gather your people around the content you care about.</p>
        <a href="/r/create" class="btn btn-primary btn-sm">Create a room</a>
      </div>
    </aside>
  </section>
{:else}
  <!-- ═══ GUEST LANDING ═══ -->

  <TopNav variant="marketing" />

  <div class="landing">

    <!-- ═══ HERO ═══ -->
    <section class="lp-hero">
      <h1>
        A book club that <span class="quiet">doesn't get lost in the group chat.</span>
      </h1>
      <p class="dek">
        Highlighter is a private reading room for small groups. Pick a book, podcast, or
        article. Highlight passages as you read. Comments stick to the highlight — not to
        4,000 messages of chat scroll.
      </p>
      <div class="cta-row">
        <a href="/onboarding" class="lp-btn lp-btn-primary">Start a room</a>
        <a href="#how" class="lp-btn lp-btn-ghost">See how it works</a>
      </div>
    </section>

    <!-- ═══ THE PROOF ═══ -->
    <section id="how" class="proof">
      <div class="mock">
        <div class="mock-head">
          <span><span class="dot"></span>The Last Thursday Club</span>
          <span>47 highlights · 12 comments</span>
        </div>
        <div class="passage">
          <div class="passage-meta">Middlemarch · George Eliot · ch. 86</div>
          <p class="passage-text">
            Her finely touched spirit had still its fine issues, though they were not widely visible.
            Her full nature spent itself in channels which had no great name on the earth. But
            <span class="hl hl-amber">the effect of her being on those around her was incalculably diffusive</span>:
            for the growing good of the world is partly dependent on unhistoric acts; and that things
            are not so ill with you and me as they might have been, is half owing to the number who
            <span class="hl hl-sage">lived faithfully a hidden life</span>.
          </p>
          <div class="thread">
            <div class="msg">
              <span class="dot-mem md-2">RT</span>
              <span><b>Rhea</b>"Unhistoric." Not <em>a</em>historic. Actively kept out of the record.</span>
            </div>
            <div class="msg">
              <span class="dot-mem md-1">MC</span>
              <span><b>M.</b>The thesis arriving 800 pages late, somehow on time.</span>
            </div>
          </div>
        </div>
      </div>
      <p class="lp-caption">Highlights and comments stay attached to the passage. So next week, you can still find them.</p>
    </section>

    <!-- ═══ SPECTRUM ═══ -->
    <section class="spectrum">
      <h2>Books, podcasts, articles, videos. <span class="muted">If it's long, it works.</span></h2>
      <div class="chips">
        <div class="chip">
          <div class="chip-kind">Book</div>
          <div class="chip-title">Middlemarch</div>
          <div class="chip-excerpt">…<em>incalculably diffusive</em>…</div>
          <div class="chip-foot">3 highlights · 2 comments</div>
        </div>
        <div class="chip">
          <div class="chip-kind">Podcast · 24:11</div>
          <div class="chip-title">On the Long Now</div>
          <div class="chip-excerpt">"…<em>each week is the only week</em>…"</div>
          <div class="chip-foot">2 highlights · 4 comments</div>
        </div>
        <div class="chip">
          <div class="chip-kind">Essay</div>
          <div class="chip-title">Purple Text, Orange Highlights</div>
          <div class="chip-excerpt">…<em>a solitary act that yearns to be social</em>…</div>
          <div class="chip-foot">6 highlights · 8 comments</div>
        </div>
        <div class="chip">
          <div class="chip-kind">Voice note · 2:14</div>
          <div class="chip-title">SK on the Eliot thing</div>
          <div class="chip-excerpt">"…<em>unhistoric acts</em> — the most important phrase she wrote."</div>
          <div class="chip-foot">5 highlights · 6 comments</div>
        </div>
      </div>
    </section>

    <!-- ═══ TENETS ═══ -->
    <section class="tenets">
      <div class="tenet">
        <div class="tenet-num">01</div>
        <div class="tenet-body">
          <h3>Small on purpose.</h3>
          <p>Six to fifteen people. Invite-only. No followers, no public profiles, no strangers.</p>
        </div>
      </div>
      <div class="tenet">
        <div class="tenet-num">02</div>
        <div class="tenet-body">
          <h3>No feed. No algorithm.</h3>
          <p>Just what your room is reading right now. That's the whole interface.</p>
        </div>
      </div>
      <div class="tenet">
        <div class="tenet-num">03</div>
        <div class="tenet-body">
          <h3>Your data, yours.</h3>
          <p>Built on Nostr. If Highlighter shuts down, your rooms and highlights move with you.</p>
        </div>
      </div>
    </section>

    <!-- ═══ FINAL CTA ═══ -->
    <section class="final">
      <h2>Start a room. Invite a few friends. Pick something to read.</h2>
      <p>It takes a minute.</p>
      <a href="/onboarding" class="lp-btn lp-btn-primary lg">Start a room</a>
    </section>
  </div>

  <Footer variant="marketing" />
{/if}

<style>
  /* ── Skeleton shimmer animation ── */
  .skeleton-card {
    height: 10rem;
    border-radius: 1rem;
    background: linear-gradient(110deg, var(--color-base-200) 30%, var(--color-base-100) 50%, var(--color-base-200) 70%);
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
  }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* ── Landing ── */
  .landing {
    max-width: 1080px;
    margin: 0 auto;
    padding: 0 20px;
    font-family: 'Inter', system-ui, -apple-system, sans-serif;
    color: #15130F;
  }

  .landing :global(*) {
    overflow-wrap: anywhere;
    word-break: normal;
  }

  @media (min-width: 720px) {
    .landing { padding: 0 32px; }
  }

  .lp-btn {
    display: inline-flex;
    align-items: center;
    text-decoration: none;
    border-radius: 4px;
    transition: background 180ms ease, border-color 180ms ease;
    font-size: 15px;
    font-weight: 500;
    line-height: 1;
  }

  .lp-btn-primary {
    padding: 14px 22px;
    background: #15130F;
    color: #F7F3EB;
  }

  .lp-btn-primary:hover { background: #C24D2C; }
  .lp-btn-primary.lg { padding: 16px 32px; font-size: 16px; }

  .lp-btn-ghost {
    padding: 14px 6px;
    color: #3A362E;
    border-bottom: 1px solid transparent;
    border-radius: 0;
  }

  .lp-btn-ghost:hover { border-bottom-color: #15130F; }

  /* ── Hero ── */
  .lp-hero {
    display: block;
    padding: 56px 0 48px;
    text-align: left;
  }

  @media (min-width: 720px) {
    .lp-hero { padding: 96px 0 72px; text-align: center; }
  }

  .lp-hero h1 {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 28px;
    line-height: 1.18;
    letter-spacing: -0.02em;
    margin: 0 0 18px;
    color: #15130F;
    text-wrap: balance;
  }

  @media (min-width: 420px) {
    .lp-hero h1 { font-size: 34px; }
  }

  @media (min-width: 720px) {
    .lp-hero h1 { font-size: 56px; line-height: 1.05; margin-bottom: 24px; }
  }

  @media (min-width: 1024px) {
    .lp-hero h1 { font-size: 68px; }
  }

  .lp-hero h1 .quiet {
    color: #C24D2C;
  }

  .lp-hero .dek {
    font-size: 17px;
    line-height: 1.5;
    color: #3A362E;
    max-width: 42ch;
    margin: 0 0 32px;
  }

  @media (min-width: 720px) {
    .lp-hero .dek { font-size: 19px; margin: 0 auto 36px; }
  }

  .cta-row {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
  }

  @media (min-width: 720px) {
    .cta-row { justify-content: center; }
  }

  /* ── Proof ── */
  .proof {
    padding: 24px 0 64px;
  }

  @media (min-width: 720px) {
    .proof { padding: 24px 0 88px; }
  }

  .mock {
    width: 100%;
    max-width: 100%;
    min-width: 0;
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    border-radius: 6px;
    box-shadow: 0 24px 64px -28px rgba(21, 19, 15, 0.22);
  }

  .mock-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid #E5DEC9;
    font-family: 'JetBrains Mono', monospace;
    font-size: 9.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: #7A7468;
    flex-wrap: wrap;
  }

  .mock-head > span { min-width: 0; }

  @media (min-width: 720px) {
    .mock-head { padding: 14px 24px; font-size: 11px; gap: 12px; }
  }

  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #C24D2C;
    margin-right: 8px;
    vertical-align: middle;
  }

  .passage {
    padding: 20px 16px;
  }

  @media (min-width: 720px) {
    .passage { padding: 36px 40px 32px; }
  }

  .passage-meta {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #7A7468;
    margin-bottom: 14px;
  }

  /* The passage uses Fraunces — this is the artifact (a book quote). */
  .passage-text {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 400;
    font-size: 17px;
    line-height: 1.65;
    margin: 0 0 20px;
    color: #15130F;
  }

  @media (min-width: 720px) {
    .passage-text { font-size: 19px; line-height: 1.7; }
  }

  .passage-text .hl {
    padding: 1px 3px;
    margin: 0 -1px;
    border-radius: 2px;
  }

  .hl-sage { background: #C8D4B5; }
  .hl-amber { background: #F5D896; }

  .thread {
    padding: 14px 14px;
    background: #EFE9DC;
    border-left: 3px solid #E8B96A;
    border-radius: 0 4px 4px 0;
    display: grid;
    gap: 12px;
  }

  @media (min-width: 720px) {
    .thread { padding: 16px 18px; }
  }

  .msg {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
    font-size: 14px;
    line-height: 1.5;
    color: #15130F;
  }

  .msg > span:last-child { word-wrap: break-word; }
  .msg b { font-weight: 600; margin-right: 4px; }

  .dot-mem {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    font-size: 10px;
    font-weight: 600;
    text-align: center;
    line-height: 24px;
    color: #15130F;
  }

  .md-1 { background: #F5D896; }
  .md-2 { background: #C8D4B5; }

  .lp-caption {
    margin: 20px auto 0;
    text-align: center;
    font-size: 14px;
    color: #7A7468;
    max-width: 50ch;
    line-height: 1.5;
  }

  /* ── Spectrum ── */
  .spectrum {
    padding: 56px 0;
    border-top: 1px solid #E5DEC9;
  }

  @media (min-width: 720px) {
    .spectrum { padding: 80px 0; }
  }

  .spectrum h2 {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 22px;
    line-height: 1.25;
    letter-spacing: -0.015em;
    margin: 0 0 24px;
    color: #15130F;
  }

  @media (min-width: 720px) {
    .spectrum h2 {
      font-size: 32px;
      text-align: center;
      margin-bottom: 40px;
      line-height: 1.2;
    }
  }

  .spectrum h2 .muted {
    color: #7A7468;
    font-weight: 400;
  }

  .chips {
    display: grid;
    grid-template-columns: 1fr;
    gap: 12px;
  }

  @media (min-width: 560px) {
    .chips { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }

  @media (min-width: 900px) {
    .chips { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  }

  .chip {
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    border-radius: 4px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: border-color 180ms ease;
  }

  .chip:hover { border-color: #E8B96A; }

  .chip-kind {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #C24D2C;
    font-weight: 500;
  }

  .chip-title {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 16px;
    line-height: 1.25;
    margin-top: 2px;
    letter-spacing: -0.005em;
  }

  .chip-excerpt {
    font-size: 13.5px;
    line-height: 1.5;
    color: #3A362E;
    padding: 8px 0 0;
    flex: 1;
  }

  .chip-excerpt em {
    background: #F5D896;
    font-style: normal;
    padding: 0 3px;
    color: #15130F;
  }

  .chip-foot {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.04em;
    color: #7A7468;
    text-transform: uppercase;
    padding-top: 10px;
    border-top: 1px dotted #D9D2BF;
  }

  /* ── Tenets ── */
  .tenets {
    padding: 56px 0;
    border-top: 1px solid #E5DEC9;
    display: grid;
    grid-template-columns: 1fr;
    gap: 28px;
  }

  @media (min-width: 720px) {
    .tenets {
      padding: 80px 0;
      grid-template-columns: repeat(3, 1fr);
      gap: 32px;
    }
  }

  .tenet {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }

  .tenet-num {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.14em;
    color: #C24D2C;
    font-weight: 500;
    padding-top: 5px;
  }

  .tenet h3 {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 18px;
    line-height: 1.3;
    margin: 0 0 6px;
    letter-spacing: -0.005em;
  }

  .tenet p {
    margin: 0;
    font-size: 14.5px;
    line-height: 1.55;
    color: #3A362E;
  }

  /* ── Final ── */
  .final {
    padding: 64px 0 80px;
    border-top: 1px solid #E5DEC9;
    text-align: center;
  }

  @media (min-width: 720px) {
    .final { padding: 88px 0 112px; }
  }

  .final h2 {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 32px;
    line-height: 1.15;
    letter-spacing: -0.02em;
    margin: 0 0 12px;
  }

  @media (min-width: 720px) {
    .final h2 { font-size: 44px; }
  }

  .final p {
    font-size: 16px;
    color: #7A7468;
    margin: 0 0 28px;
    line-height: 1.5;
  }
</style>
