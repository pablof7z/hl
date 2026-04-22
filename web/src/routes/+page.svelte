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
  import { buildJoinedCommunities, groupIdFromEvent } from '$lib/ndk/groups';
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
      ? buildJoinedCommunities(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
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

  /* ── Landing: generate waveform bars deterministically ── */
  function buildWaveformBars(): number[] {
    const bars: number[] = [];
    for (let i = 0; i < 200; i++) {
      const seed = Math.sin(i * 0.7) * Math.cos(i * 0.3) + Math.sin(i * 0.13) * 0.5;
      bars.push(Math.max(8, Math.min(60, 30 + seed * 22)));
    }
    return bars;
  }
  const waveformBars = buildWaveformBars();
</script>

{#if signedIn}
  <!-- ═══ SIGNED-IN FEED ═══ -->
  <section class="dashboard-header">
    <div class="dashboard-header-copy">
      <h1 class="dashboard-headline">Your feed</h1>
    </div>
    <div class="dashboard-ctas">
      {#each memberActions as action (action.href)}
        <a
          href={action.href}
          class={action.tone === 'secondary' ? 'btn-secondary' : 'btn-primary'}
        >
          {action.label}
        </a>
      {/each}
    </div>
  </section>

  <section class="dashboard-body">
    <div class="feed-main">
      {#if isLoading}
        <div class="feed-skeleton">
          <div class="skeleton-card"></div>
          <div class="skeleton-card"></div>
          <div class="skeleton-card"></div>
        </div>
        <p class="feed-loading-text">Loading your feed...</p>
      {:else if isEmpty && !hasRooms && !hasFollows}
        <div class="feed-empty">
          <h2>Your feed starts here.</h2>
          <p>Join a room or follow someone to see highlights appear in your feed.</p>
          <div class="feed-empty-actions">
            <a href="/discover" class="btn-primary">Discover rooms</a>
            <a href="/r/create" class="btn-secondary">Create a room</a>
          </div>
        </div>
      {:else if isEmpty && hasRooms}
        <div class="feed-empty">
          <h2>Your rooms are quiet.</h2>
          <p>No highlights have been shared yet. Be the first.</p>
          <div class="feed-empty-actions">
            <a href="/rooms" class="btn-primary">Visit your rooms</a>
          </div>
        </div>
      {:else if isEmpty && hasFollows}
        <div class="feed-empty">
          <h2>Nothing new from your network.</h2>
          <p>The people you follow haven't shared highlights recently.</p>
          <div class="feed-empty-actions">
            <a href="/discover" class="btn-primary">Discover rooms</a>
          </div>
        </div>
      {:else}
        <div class="feed-groups">
          {#each visibleGroups as group (group.referenceKey)}
            <HighlightSourceGroup {group} {rooms} showShareControl={true} />
          {/each}
        </div>
        {#if !showAll && feedGroups.length > INITIAL_GROUP_COUNT}
          <button class="btn-secondary feed-show-more" onclick={() => showAll = true}>
            Show more ({feedGroups.length - INITIAL_GROUP_COUNT} remaining)
          </button>
        {/if}
        {#if resolvingArtifacts}
          <p class="feed-resolving">Resolving source details...</p>
        {/if}
      {/if}
    </div>

    <aside class="feed-rail">
      {#if rooms.length > 0}
        <div class="rail-section">
          <h3 class="rail-heading">Your rooms</h3>
          <ul class="rail-circle-list">
            {#each rooms.slice(0, 6) as room (room.id)}
              <li>
                <a href="/r/{room.id}" class="rail-circle-link">{room.name}</a>
              </li>
            {/each}
          </ul>
          {#if rooms.length > 6}
            <a href="/rooms" class="rail-view-all">View all rooms</a>
          {/if}
        </div>
      {/if}
      <div class="rail-cta-card">
        <h3>Start a new room</h3>
        <p>Gather your people around the content you care about.</p>
        <a href="/r/create" class="btn-primary">Create a room</a>
      </div>
    </aside>
  </section>
{:else}
  <!-- ═══ GUEST — THE ANNOTATION LANDING ═══ -->

  <TopNav variant="marketing" />

  <div class="landing-page">

    <!-- ═══ HERO ═══ -->
    <section class="landing-hero landing-section">
      <div class="main">
        <h1>Read together, <em><mark>quietly.</mark></em></h1>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">quietly</div>
          <div class="anno-pen">tired of loud. <u>good word.</u></div>
        </div>
      </div>

      <div class="main">
        <p class="hero-dek">
          Highlighter is a shared spine for the books, essays, and podcasts a small circle of friends
          are reading this week. One room. A few people. <mark>Every conversation next to the passage it's about.</mark>
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">every conversation next to the passage it's about</div>
          <div class="anno-pen">ok. <u>this is the problem.</u> our whatsapp is 4,000 messages deep and nobody can find anything anjali said about the eliot book anymore.</div>
        </div>
      </div>

      <div class="full">
        <div class="hero-ctas">
          <a href="/onboarding" class="landing-btn-primary">Join</a>
        </div>
      </div>
    </section>

    <!-- ═══ WHAT IT IS ═══ -->
    <section id="what" class="landing-section">
      <div class="main">
        <h2 class="sec-head">A book club that <em><mark>doesn't die in a group chat.</mark></em></h2>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">doesn't die in a group chat</div>
          <div class="anno-pen">called out.<br /><span class="strike">1984-2026.</span></div>
        </div>
      </div>

      <div class="main">
        <p class="sec-lead">
          Six to fifteen people <mark>who share taste</mark>. One shared spine for what you're all reading,
          watching, or listening to. Highlights sit next to the passage. Conversation attaches to the sentence it's about.
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">who share taste</div>
          <div class="anno-pen">so: M, J, T, S, and probably R if she's free.</div>
        </div>
      </div>

      <div class="main">
        <p class="body-copy">
          You invite your people. Someone in the room shares the book, the essay, the podcast episode.
          As each member reads or listens, they mark what catches them — an excerpt, a paragraph,
          <mark>a timestamp in an audio file</mark>. Everyone sees everyone's marks.
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">a timestamp in an audio file</div>
          <div class="anno-pen">wait — the podcast thing??<br /><span class="anno-pen tiny">(keep reading keep reading)</span></div>
        </div>
      </div>

      <div class="main">
        <p class="body-copy">
          <strong>No feed.</strong> No discovery tab. <mark>No strangers grading your takes.</mark>
          No algorithm picking what the room reads. The surface is whatever your six people are into this week, and nothing else.
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">No strangers grading your takes</div>
          <div class="anno-pen"><span class="emph">god yes.</span></div>
        </div>
      </div>
    </section>

    <!-- ═══ HOW IT WORKS (BOOK UI) ═══ -->
    <section class="landing-section">
      <div class="main">
        <h2 class="sec-head">The book stays <em>pinned.</em> The margins fill up.</h2>
      </div>
      <div class="marg"></div>

      <div class="main">
        <p class="body-copy">
          Every room anchors to <mark>one artifact at a time</mark> — the piece of content you're all currently reading.
          Highlights from every member appear against the passages themselves, each in their own color. Comments attach
          to the excerpt or the sentence — <mark>not a floating thread that scrolls away from what it's about.</mark>
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">one artifact at a time</div>
          <div class="anno-pen">we have four books <u>half-read</u> across the group at once. always.</div>
        </div>
        <div style="height:28px;"></div>
        <div class="anno">
          <div class="anno-quote">not a floating thread that scrolls away</div>
          <div class="anno-pen">this is the first screenshot i've seen of a reading app that looks like what reading-with-someone feels like in my head.</div>
        </div>
      </div>

      <div class="full">
        <div class="mock-wrap">
          <div class="mock-header">
            <div><span class="mock-dot"></span>The Last Thursday Club · week 3</div>
            <div>✎ 47 highlights · 12 threads</div>
          </div>
          <div class="book-card">
            <div class="book-cover">
              <div class="bc-top">— a novel —</div>
              <div>
                <div class="bc-title">Middlemarch</div>
                <div class="bc-author">George Eliot</div>
              </div>
            </div>
            <div class="book-meta-area">
              <h4>Middlemarch</h4>
              <div class="author">George Eliot · 1871</div>
              <div class="book-stats">
                <span><b>6</b> reading</span>
                <span><b>47</b> highlights</span>
                <span><b>12</b> threads</span>
              </div>
              <div class="members-row">
                <span class="member-dot md-1">MC</span>
                <span class="member-dot md-2 overlap">RT</span>
                <span class="member-dot md-3 overlap">JO</span>
                <span class="member-dot md-4 overlap">AN</span>
                <span class="member-dot md-5 overlap">DL</span>
                <span class="member-dot md-6 overlap">SK</span>
                <span class="members-count">all 6 active this week</span>
              </div>
            </div>
          </div>

          <div class="passage-mock">
            <div class="passage-meta">From chapter 86 · last page of the novel</div>
            <p class="passage-text">
              Her finely touched spirit had still its fine issues, though they were not widely visible.
              Her full nature, like that river of which Cyrus broke the strength, spent itself in channels
              which had no great name on the earth. But <span class="hl hl-amber">the effect of her being on those around her was incalculably diffusive</span>:
              for the growing good of the world is partly dependent on unhistoric acts; and that things are
              not so ill with you and me as they might have been, is half owing to the number who
              <span class="hl hl-sage">lived faithfully a hidden life</span>, and rest in unvisited tombs.
            </p>
            <div class="thread-mock">
              <div class="thread-msg">
                <div class="member-dot md-2">RT</div>
                <div class="thread-msg-body">
                  <span class="name">Rhea T.</span>
                  I've read this paragraph a dozen times since 2012 and only now noticed "unhistoric." Not ahistoric. Unhistoric. Actively kept out of the record.
                  <span class="time">· Mon 9:12</span>
                </div>
              </div>
              <div class="thread-msg">
                <div class="member-dot md-1">MC</div>
                <div class="thread-msg-body">
                  <span class="name">M. Costa</span>
                  The thesis of the novel arriving 800 pages late and somehow on time.
                  <span class="time">· Mon 9:44</span>
                </div>
              </div>
              <div class="thread-msg">
                <div class="member-dot md-4">AN</div>
                <div class="thread-msg-body">
                  <span class="name">Anouk</span>
                  "that things are not so ill with you and me" — she's <i>addressing</i> you. The pronoun slide is the whole point.
                  <span class="time">· Tue 6:08</span>
                </div>
              </div>
              <div class="reply-prompt">reply · or highlight the next passage →</div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- ═══ NOT JUST BOOKS ═══ -->
    <section id="media" class="landing-section">
      <div class="main">
        <h2 class="sec-head">Not <em><mark>just books.</mark></em></h2>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">just books</div>
          <div class="anno-pen">oh. so podcasts too??</div>
        </div>
      </div>

      <div class="full">
        <div class="chip-grid">
          <div class="chip">
            <div class="chip-type"><b>Book</b> · chapter 86</div>
            <div class="chip-title">Middlemarch</div>
            <div class="chip-source">George Eliot</div>
            <div class="chip-excerpt">…the effect of her being on those around her was <span class="inner-hl">incalculably diffusive</span>…</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-1">MC</span>
                <span class="member-dot md-2 overlap">RT</span>
                <span class="member-dot md-4 overlap">AN</span>
              </div>
              <span class="chip-count">3 marked · 2 replies</span>
            </div>
          </div>

          <div class="chip">
            <div class="chip-type"><b>Podcast</b> · 24:11 → 25:40</div>
            <div class="chip-title">On the Long Now</div>
            <div class="chip-source">The Ezra Klein Show · ep. 487</div>
            <div class="chip-excerpt">"…the embarrassing thing about the internet — it behaves as if <span class="inner-hl">each week is the only week</span>."</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-1">MC</span>
                <span class="member-dot md-2 overlap">RT</span>
              </div>
              <span class="chip-count">2 marked · 4 replies</span>
            </div>
          </div>

          <div class="chip">
            <div class="chip-type"><b>Essay</b> · dergigi.com · 2023</div>
            <div class="chip-title">Purple Text, Orange Highlights</div>
            <div class="chip-source">Dergigi</div>
            <div class="chip-excerpt">…reading is a solitary act <span class="inner-hl">that yearns to be a social one</span>…</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-1">MC</span>
                <span class="member-dot md-2 overlap">RT</span>
                <span class="member-dot md-3 overlap">JO</span>
                <span class="member-dot md-4 overlap">AN</span>
                <span class="member-dot md-5 overlap">DL</span>
                <span class="member-dot md-6 overlap">SK</span>
              </div>
              <span class="chip-count">all 6 · 8 replies</span>
            </div>
          </div>

          <div class="chip">
            <div class="chip-type"><b>Video</b> · 47:30 → 48:15</div>
            <div class="chip-title">At the Long Now Foundation</div>
            <div class="chip-source">Stewart Brand · lecture · 02:14:00</div>
            <div class="chip-excerpt">"A civilization is a <span class="inner-hl">slow literature</span>. Every institution is a paragraph…"</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-4">AN</span>
                <span class="member-dot md-5 overlap">DL</span>
              </div>
              <span class="chip-count">2 marked · 1 reply</span>
            </div>
          </div>

          <div class="chip">
            <div class="chip-type"><b>PDF</b> · ch. 2 · pp. 19</div>
            <div class="chip-title">Working in Public</div>
            <div class="chip-source">Nadia Eghbal · Stripe Press</div>
            <div class="chip-excerpt">…<span class="inner-hl">the few who actually contribute</span>, and the many who watch…</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-2">RT</span>
                <span class="member-dot md-3 overlap">JO</span>
                <span class="member-dot md-6 overlap">SK</span>
              </div>
              <span class="chip-count">3 marked · 2 replies</span>
            </div>
          </div>

          <div class="chip">
            <div class="chip-type"><b>Voice memo</b> · 2:14 → 2:47</div>
            <div class="chip-title">SK — on the Eliot thing</div>
            <div class="chip-source">Sun · 2am · recorded to the room</div>
            <div class="chip-excerpt">"…<span class="inner-hl">unhistoric acts</span> is maybe the most important phrase she wrote and we didn't talk about it enough…"</div>
            <div class="chip-foot">
              <div class="dots">
                <span class="member-dot md-1">MC</span>
                <span class="member-dot md-2 overlap">RT</span>
                <span class="member-dot md-3 overlap">JO</span>
                <span class="member-dot md-4 overlap">AN</span>
                <span class="member-dot md-5 overlap">DL</span>
              </div>
              <span class="chip-count">5 marked · 6 replies</span>
            </div>
          </div>
        </div>
      </div>

      <div class="main">
        <p class="body-copy">
          And here's one of them worked out in full.
          <mark>Mark the thirty seconds you wanted back. Your friend's reply lives at the timestamp.</mark>
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">mark the thirty seconds you wanted back</div>
          <div class="anno-pen big">WAIT.</div>
          <div class="anno-pen">every tuesday M. and i get on a call and one of us says "do you remember when he said that thing about—" and <u>neither of us does.</u></div>
          <div class="anno-pen tiny">this solves it.</div>
        </div>
      </div>

      <div class="full">
        <div class="podcast-card">
          <div class="mock-header">
            <div><span class="mock-dot"></span>This week's listen · 4 of 6 members in</div>
            <div>✎ 11 highlights · 3 threads</div>
          </div>
          <div class="podcast-head">
            <div class="podcast-artwork"><em>Long Now</em></div>
            <div class="podcast-meta">
              <h5>On the Long Now, deep time, and the civilizational cost of short attention</h5>
              <div class="show">The Ezra Klein Show · ep. 487 · with Stewart Brand</div>
              <div class="dur">1 hr 47 min · released Apr 9</div>
            </div>
          </div>

          <div class="waveform-wrap">
            <div class="hl-span s1"><span class="hl-span-label">24:11 → 25:40</span></div>
            <div class="hl-span s2"><span class="hl-span-label">1:08:22 → 1:09:15</span></div>
            <div class="waveform">
              {#each waveformBars as h, i (i)}
                <span class="bar" style:height="{h}%"></span>
              {/each}
            </div>
          </div>
          <div class="waveform-timeline">
            <span>0:00</span><span>30:00</span><span>1:00:00</span><span>1:30:00</span><span>1:47:00</span>
          </div>

          <div class="podcast-highlight">
            <div class="ph-stamp"><b>00:24:11 → 00:25:40</b> · marked by M. Costa, Rhea T.</div>
            <p class="ph-quote">"The thing the 10,000-year clock is trying to teach you is that your generation is not the last generation. It's the embarrassing thing about the internet — the internet behaves as if each week is the only week."</p>
            <div class="ph-reaction">
              <div class="member-dot md-2">RT</div>
              <div class="pr-body">
                <span class="pr-name">Rhea T.</span>
                This is the line worth the whole episode. I want to put it on a poster. Also — someone please tell me we aren't going to forget this by Sunday.
              </div>
            </div>
          </div>

          <div class="podcast-highlight">
            <div class="ph-stamp"><b>01:08:22 → 01:09:15</b> · marked by Anouk</div>
            <p class="ph-quote">"A civilization is a slow literature. Every institution is a paragraph someone wrote that other people then took seriously."</p>
            <div class="ph-reaction">
              <div class="member-dot md-4">AN</div>
              <div class="pr-body">
                <span class="pr-name">Anouk</span>
                The line about "slow literature" rhymes hard with the Middlemarch passage we were on last week that I almost emailed the room. Opening a thread.
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- ═══ THE ROOM ═══ -->
    <section id="room" class="landing-section">
      <div class="full">
        <div class="sec-kicker">— the room</div>
      </div>

      <div class="main">
        <h2 class="sec-head">Small on purpose. <em><mark>Invitation by design.</mark></em></h2>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">Invitation by design</div>
          <div class="anno-pen">good. don't want randos.</div>
        </div>
      </div>

      <div class="main">
        <p class="sec-lead">
          Rooms are closed by default. Six to twenty members. Most rooms stay <mark>around a dozen people.</mark>
          Big enough that someone is always the interesting one. Small enough that you know every voice.
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">around a dozen people</div>
          <div class="anno-pen">5 feels right for us.<br /><span class="anno-pen tiny">maybe start at 3 and grow. smarter.</span></div>
        </div>
      </div>

      <div class="main">
        <p class="body-copy">
          You can make a room public for read-only discovery (highlights and threads visible to the world,
          participation stays invitation-only), or keep it fully private for family, work, or
          <mark>the kind of conversation that should stay in the room.</mark>
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">the kind of conversation that should stay in the room</div>
          <div class="anno-pen">family book club version = <u>great.</u></div>
        </div>
      </div>

      <div class="full">
        <div class="room-header-mock">
          <div class="room-label">Room</div>
          <h5 class="room-name">The Last Thursday Club<em>.</em></h5>
          <div class="room-stats">
            <span><b>6</b> members</span>
            <span><b>Closed</b> — by invitation</span>
            <span><b>Private</b> — only members</span>
            <span><b>Reading since</b> Jan 2024</span>
          </div>
          <div class="room-activity"><b>Currently:</b> Middlemarch · week 3 of 4. Also this week's listen: Ezra Klein on the Long Now.</div>
          <div class="room-activity"><b>Up next:</b> a shortlist the room is voting on — Le Guin's essays, <i>Mrs Dalloway</i>, the Calvino memos. Voting closes Sunday.</div>
        </div>
      </div>
    </section>

    <!-- ═══ OWNERSHIP ═══ -->
    <section class="landing-section">
      <div class="full">
        <div class="sec-kicker">— ownership</div>
      </div>

      <div class="main">
        <h2 class="sec-head">Your rooms <em><mark>aren't trapped here.</mark></em></h2>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">aren't trapped here</div>
          <div class="anno-pen">last reading app i used shut down in october.<br /><span class="anno-pen tiny">lost everything. still annoyed.</span></div>
        </div>
      </div>

      <div class="main">
        <p class="body-copy">
          Highlighter runs on Nostr — an open protocol where your identity is a key you own, your rooms live on
          a host you can change, and your highlights are portable events anyone can read. The technical details only
          matter if we go dark. If we ever do, <mark>your rooms move to another host and nobody loses a thing.</mark>
        </p>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">your rooms move to another host and nobody loses a thing</div>
          <div class="anno-pen">ok. that's <u>unusually honest</u> for a product page.</div>
        </div>
      </div>

      <div class="full">
        <div class="proto-callout">
          <div class="proto-key">⌇</div>
          <div class="proto-text">
            <h5>One key per reader. One protocol. No silo.</h5>
            <p>Your Nostr keypair is your identity. Your rooms are portable between hosts. Your highlights export to any compatible client. Built on <em>NIP-29</em> groups and <em>NIP-84</em> highlights. Interoperable by default.</p>
          </div>
        </div>
      </div>
    </section>

    <!-- ═══ FINAL CTA ═══ -->
    <section class="landing-section">
      <div class="full">
        <div class="sec-kicker">— joining</div>
      </div>

      <div class="main">
        <h2 class="sec-head">Start a room. Pick your <em><mark>first three.</mark></em></h2>
      </div>
      <div class="marg">
        <div class="anno">
          <div class="anno-quote">first three</div>
          <div class="anno-pen">M. and J. for sure. T. if she's around.</div>
        </div>
      </div>

      <div class="main">
        <p class="sec-lead">
          Set up your key, make your first room, and invite three or four people you'd actually want
          to read a difficult book with.
        </p>
        <div class="final-cta-buttons">
          <a href="/onboarding" class="landing-btn-primary">Join</a>
        </div>
        <p class="final-fine">
          Not a broadcast list. Not "everyone who's ever read a book with me." The right three.
        </p>
      </div>
      <div class="marg">
        <div class="anno free">
          <div class="anno-pen big">ok.<br />doing it.</div>
          <div class="anno-pen tiny">texting M. and J. now.</div>
        </div>
      </div>
    </section>
  </div>

  <Footer variant="marketing" />
{/if}

<style>
  /* ═══════════════════════════════════════════
     SIGNED-IN FEED (unchanged)
     ═══════════════════════════════════════════ */

  .dashboard-header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.5rem 0 0;
  }

  .dashboard-headline {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.5rem, 3.5vw, 2rem);
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .dashboard-ctas {
    display: flex;
    flex-wrap: wrap;
    gap: 0.625rem;
    flex-shrink: 0;
  }

  .dashboard-body {
    display: grid;
    grid-template-columns: 1fr 22rem;
    gap: 2rem;
    align-items: start;
  }

  .feed-main { min-width: 0; }

  .feed-groups {
    display: grid;
    gap: 0.9rem;
  }

  .feed-show-more {
    margin-top: 1rem;
    width: 100%;
    text-align: center;
    border: none;
    font-family: inherit;
  }

  .feed-loading-text, .feed-resolving {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .feed-resolving { margin-top: 0.75rem; }

  .feed-skeleton {
    display: grid;
    gap: 0.9rem;
  }

  .skeleton-card {
    height: 10rem;
    border-radius: 1rem;
    background: linear-gradient(110deg, var(--surface-soft) 30%, var(--surface) 50%, var(--surface-soft) 70%);
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
  }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  .feed-empty {
    display: grid;
    gap: 0.5rem;
    padding: 2.5rem 2rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1rem;
    background: var(--surface);
    text-align: center;
  }

  .feed-empty h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.35rem;
  }

  .feed-empty p {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.6;
  }

  .feed-empty-actions {
    display: flex;
    justify-content: center;
    gap: 0.75rem;
    margin-top: 1rem;
  }

  .feed-rail {
    display: grid;
    gap: 1.25rem;
  }

  .rail-section {
    display: grid;
    gap: 0.5rem;
  }

  .rail-heading {
    margin: 0;
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .rail-circle-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.125rem;
  }

  .rail-circle-link {
    display: block;
    padding: 0.45rem 0.625rem;
    border-radius: 0.5rem;
    color: var(--text-strong);
    font-size: 0.9rem;
    text-decoration: none;
    transition: background 120ms ease;
  }

  .rail-circle-link:hover { background: var(--surface-soft); }

  .rail-view-all {
    font-size: 0.813rem;
    color: var(--accent);
    text-decoration: none;
    padding-left: 0.625rem;
  }

  .rail-view-all:hover { text-decoration: underline; }

  .rail-cta-card {
    display: grid;
    gap: 0.5rem;
    padding: 1.25rem 0 0;
    border-top: 1px solid var(--color-base-300);
  }

  .rail-cta-card h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-strong);
  }

  .rail-cta-card p {
    margin: 0;
    font-size: 0.875rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .btn-primary {
    display: inline-block;
    background: var(--accent);
    color: #fff;
    font-size: 0.94rem;
    font-weight: 500;
    padding: 0.85rem 1.75rem;
    border-radius: 9999px;
    transition: background 0.18s ease, transform 0.12s ease;
    cursor: pointer;
    white-space: nowrap;
    text-decoration: none;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
  }

  .btn-secondary {
    display: inline-block;
    background: var(--surface);
    color: var(--text-strong);
    font-size: 0.94rem;
    font-weight: 500;
    padding: 0.85rem 1.75rem;
    border-radius: 9999px;
    border: 1px solid var(--color-base-300);
    cursor: pointer;
    white-space: nowrap;
    text-decoration: none;
  }

  @media (max-width: 820px) {
    .dashboard-header {
      flex-direction: column;
      align-items: flex-start;
    }
    .dashboard-body { grid-template-columns: 1fr; }
    .feed-rail { order: 1; }
  }

  /* ═══════════════════════════════════════════
     LANDING (annotation direction, round 02)
     ═══════════════════════════════════════════ */

  .landing-page {
    max-width: 1280px;
    margin: 0 auto;
    padding: 0 40px;
    background: var(--bg);
    font-family: var(--font-sans);
    color: var(--ink);
    font-size: 17px;
    line-height: 1.6;
  }

  @media (max-width: 900px) {
    .landing-page { padding: 0 20px; }
  }

  .landing-section {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 48px;
    padding: 72px 0;
    align-items: start;
    position: relative;
  }

  @media (max-width: 900px) {
    .landing-section {
      grid-template-columns: 1fr;
      gap: 16px;
      padding: 48px 0;
    }
  }

  .landing-section + .landing-section {
    border-top: 1px solid #D9D2BF;
  }

  .full { grid-column: 1 / -1; }
  .main { grid-column: 1; min-width: 0; }
  .marg { grid-column: 2; }

  @media (max-width: 900px) {
    .full, .main, .marg { grid-column: 1; }
  }

  /* marker (highlight-on-page) */
  .landing-page :global(mark),
  .landing-page .mark {
    background: linear-gradient(180deg, transparent 0%, transparent 12%, #F5D896 12%, #F5D896 92%, transparent 92%);
    color: inherit;
    padding: 0 2px;
    margin: 0 -1px;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
    position: relative;
  }

  /* annotation */
  .anno {
    position: relative;
    padding: 0 0 0 18px;
    border-left: 1px dotted rgba(31, 63, 120, 0.35);
    margin-bottom: 0;
  }

  .anno::before {
    content: '◂';
    position: absolute;
    left: -6px;
    top: -2px;
    color: #1F3F78;
    font-size: 13px;
    line-height: 1;
    background: #F7F3EB;
    width: 10px;
    text-align: center;
  }

  .anno-quote {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 13.5px;
    line-height: 1.45;
    color: #3A362E;
    background: linear-gradient(180deg, transparent 55%, rgba(245, 216, 150, 0.55) 55%);
    padding: 2px 0;
    margin: 0 0 10px;
    display: inline;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }

  .anno-quote::before { content: '\201C'; }
  .anno-quote::after { content: '\201D'; }

  .anno-pen {
    font-family: 'Caveat', cursive;
    font-weight: 500;
    font-size: 22px;
    line-height: 1.25;
    color: #1F3F78;
    margin-top: 6px;
  }

  .anno-pen u {
    text-decoration-color: #1F3F78;
    text-decoration-thickness: 1.5px;
    text-underline-offset: 2px;
  }

  .anno-pen .strike {
    text-decoration: line-through;
    text-decoration-color: #1F3F78;
    color: #3A5A95;
  }

  .anno-pen.big {
    font-size: 32px;
    font-weight: 600;
    line-height: 1.1;
  }

  .anno-pen.tiny {
    display: block;
    margin-top: 4px;
    font-size: 18px;
    font-weight: 400;
    color: #3A5A95;
  }

  .anno-pen .emph {
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .anno.free {
    border-left-style: none;
    padding-left: 0;
  }

  .anno.free::before {
    content: none;
  }

  @media (max-width: 900px) {
    .anno {
      border-left: 3px solid #1F3F78;
      padding: 12px 14px;
      margin-top: -4px;
      margin-bottom: 18px;
      background: rgba(31, 63, 120, 0.04);
      border-radius: 4px;
    }
    .anno::before { display: none; }
  }

  /* Hero */
  .landing-hero { padding: 72px 0 96px; }

  .hero-kicker {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: #C24D2C;
    margin-bottom: 28px;
  }

  .landing-page h1 {
    font-family: 'Fraunces', serif;
    font-weight: 300;
    font-size: clamp(44px, 7vw, 88px);
    line-height: 1.02;
    letter-spacing: -0.025em;
    color: #15130F;
    margin: 0 0 28px;
    max-width: 14ch;
  }

  .landing-page h1 em {
    font-style: italic;
    font-weight: 400;
    color: #C24D2C;
  }

  .hero-dek {
    font-family: 'Fraunces', serif;
    font-weight: 400;
    font-size: clamp(19px, 2vw, 24px);
    line-height: 1.5;
    color: #3A362E;
    max-width: 52ch;
    margin: 0 0 40px;
  }

  .hero-ctas {
    display: flex;
    gap: 18px;
    align-items: center;
    flex-wrap: wrap;
  }

  .landing-btn-primary {
    padding: 16px 28px;
    background: #15130F;
    color: #F7F3EB;
    font-family: 'Inter', sans-serif;
    font-size: 15px;
    font-weight: 500;
    letter-spacing: 0.01em;
    text-decoration: none;
    transition: background 200ms ease;
  }

  .landing-btn-primary:hover { background: #C24D2C; }

  .landing-btn-secondary {
    padding: 16px 0;
    color: #3A362E;
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 17px;
    text-decoration: underline;
    text-decoration-color: #D9D2BF;
    text-underline-offset: 5px;
  }

  .landing-btn-secondary:hover {
    color: #C24D2C;
    text-decoration-color: #C24D2C;
  }

  .sec-kicker {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: #C24D2C;
    margin-bottom: 18px;
  }

  .sec-head {
    font-family: 'Fraunces', serif;
    font-weight: 400;
    font-size: clamp(32px, 4.5vw, 52px);
    line-height: 1.08;
    letter-spacing: -0.018em;
    color: #15130F;
    margin: 0 0 28px;
    max-width: 22ch;
  }

  .sec-head em {
    font-style: italic;
    color: #C24D2C;
  }

  .sec-lead {
    font-family: 'Fraunces', serif;
    font-size: 22px;
    line-height: 1.55;
    font-style: italic;
    color: #3A362E;
    max-width: 52ch;
    margin: 0;
  }

  p.body-copy {
    font-size: 17px;
    line-height: 1.7;
    color: #3A362E;
    max-width: 58ch;
    margin: 0;
  }

  p.body-copy + p.body-copy { margin-top: 1.15em; }
  p.body-copy strong { color: #15130F; font-weight: 500; }

  /* Mock wrappers */
  .mock-wrap, .podcast-card {
    margin-top: 40px;
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    border-radius: 4px;
    padding: 28px;
    box-shadow: 0 20px 48px -24px rgba(21, 19, 15, 0.2);
  }

  .mock-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: 18px;
    border-bottom: 1px solid #D9D2BF;
    margin-bottom: 24px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #7A7468;
  }

  .mock-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #C24D2C;
    margin-right: 8px;
  }

  .book-card {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 24px;
    align-items: start;
  }

  .book-cover {
    aspect-ratio: 2/3;
    background: linear-gradient(135deg, #3A2416 0%, #5A3A22 100%);
    border-radius: 2px;
    padding: 18px 14px;
    color: #E6D9BC;
    font-family: 'Fraunces', serif;
    font-weight: 400;
    font-size: 13px;
    line-height: 1.25;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.15);
    position: relative;
  }

  .book-cover::before {
    content: '';
    position: absolute;
    top: 8px;
    bottom: 8px;
    left: 4px;
    width: 2px;
    background: rgba(230, 217, 188, 0.25);
  }

  .bc-top {
    font-style: italic;
    font-size: 11px;
    opacity: 0.7;
  }

  .bc-title {
    font-size: 16px;
    line-height: 1.1;
    margin-top: auto;
    margin-bottom: 12px;
  }

  .bc-author {
    font-style: italic;
    font-size: 11px;
    opacity: 0.85;
    border-top: 1px solid rgba(230, 217, 188, 0.3);
    padding-top: 6px;
  }

  .book-meta-area h4 {
    font-family: 'Fraunces', serif;
    font-weight: 500;
    font-size: 22px;
    line-height: 1.2;
    margin: 0 0 2px;
    color: #15130F;
  }

  .book-meta-area .author {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 15px;
    color: #7A7468;
    margin-bottom: 16px;
  }

  .book-stats {
    display: flex;
    gap: 20px;
    font-size: 13px;
    color: #7A7468;
    margin-bottom: 18px;
  }

  .book-stats b {
    color: #15130F;
    font-weight: 500;
  }

  .members-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .member-dot {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    font-family: 'Inter', sans-serif;
    font-size: 11px;
    font-weight: 500;
    text-align: center;
    line-height: 28px;
    color: #15130F;
    border: 2px solid #FFFEFA;
    display: inline-block;
  }

  .members-row .overlap { margin-left: -10px; }
  .md-1 { background: #F5D896; }
  .md-2 { background: #C8D4B5; }
  .md-3 { background: #EAC6C8; }
  .md-4 { background: #BCD0E0; }
  .md-5 { background: #D0C4E0; }
  .md-6 { background: #F5E6A8; }

  .members-count {
    font-size: 12px;
    color: #7A7468;
    margin-left: 6px;
  }

  .passage-mock {
    margin-top: 28px;
    padding: 28px 32px 32px;
    border-top: 1px solid #D9D2BF;
    border-bottom: 1px solid #D9D2BF;
  }

  .passage-meta {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #7A7468;
    margin-bottom: 12px;
  }

  .passage-text {
    font-family: 'Fraunces', serif;
    font-size: 19px;
    line-height: 1.7;
    color: #15130F;
    margin: 0 0 16px;
  }

  .passage-text .hl {
    padding: 2px 4px;
    margin: 0 -2px;
    border-radius: 2px;
  }

  .hl-sage { background: #C8D4B5; }
  .hl-amber { background: #F5D896; }

  .thread-mock {
    margin-top: 18px;
    padding: 16px 18px;
    background: #EFE9DC;
    border-left: 3px solid #E8B96A;
    border-radius: 0 2px 2px 0;
  }

  .thread-msg {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    margin-bottom: 10px;
  }

  .thread-msg:last-of-type { margin-bottom: 0; }
  .thread-msg .member-dot {
    width: 22px;
    height: 22px;
    line-height: 22px;
    font-size: 10px;
  }

  .thread-msg-body {
    font-size: 14px;
    line-height: 1.5;
    color: #15130F;
  }

  .thread-msg-body .name {
    font-weight: 500;
    margin-right: 8px;
  }

  .thread-msg-body .time {
    color: #7A7468;
    font-size: 12px;
    margin-left: 6px;
  }

  .reply-prompt {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px dashed #D9D2BF;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: #7A7468;
    letter-spacing: 0.05em;
  }

  /* Chip grid (not just books) */
  .chip-grid {
    margin-top: 20px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }

  @media (max-width: 900px) {
    .chip-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }

  @media (max-width: 560px) {
    .chip-grid { grid-template-columns: 1fr; }
  }

  .chip {
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    padding: 22px 22px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 214px;
    position: relative;
    transition: border-color 200ms ease, transform 200ms ease;
  }

  .chip:hover {
    border-color: #E8B96A;
    transform: translateY(-1px);
  }

  .chip-type {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: #7A7468;
  }

  .chip-type b {
    color: #C24D2C;
    font-weight: 500;
  }

  .chip-title {
    font-family: 'Fraunces', serif;
    font-weight: 500;
    font-size: 19px;
    line-height: 1.18;
    color: #15130F;
    letter-spacing: -0.005em;
    margin-top: 2px;
  }

  .chip-source {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 13.5px;
    color: #7A7468;
    margin-top: -4px;
    margin-bottom: 2px;
  }

  .chip-excerpt {
    font-family: 'Fraunces', serif;
    font-size: 14.5px;
    line-height: 1.55;
    color: #3A362E;
    padding: 10px 0 2px;
    border-top: 1px dotted #D9D2BF;
    flex: 1;
  }

  .inner-hl {
    background: #F5D896;
    padding: 1px 3px;
    color: #15130F;
  }

  .chip-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 10px;
    border-top: 1px dotted #D9D2BF;
    gap: 8px;
  }

  .chip-foot .dots {
    display: flex;
    align-items: center;
  }

  .chip-foot .member-dot {
    width: 22px;
    height: 22px;
    line-height: 22px;
    font-size: 10px;
  }

  .chip-count {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.04em;
    color: #7A7468;
    text-transform: uppercase;
    text-align: right;
    flex-shrink: 0;
  }

  /* Podcast card */
  .podcast-head {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: 20px;
    margin-bottom: 22px;
  }

  .podcast-artwork {
    aspect-ratio: 1/1;
    background: linear-gradient(135deg, #2A3E5E 0%, #4A6B9C 100%);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'Fraunces', serif;
    font-style: italic;
    color: #D8E3F2;
    font-size: 12px;
    text-align: center;
    padding: 8px;
    line-height: 1.1;
  }

  .podcast-meta h5 {
    font-family: 'Fraunces', serif;
    font-weight: 500;
    font-size: 20px;
    line-height: 1.25;
    margin: 0 0 4px;
    color: #15130F;
  }

  .podcast-meta .show {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 14px;
    color: #7A7468;
    margin-bottom: 8px;
  }

  .podcast-meta .dur {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #7A7468;
  }

  .waveform-wrap {
    position: relative;
    height: 68px;
    background: #EFE9DC;
    border-radius: 4px;
    margin: 24px 0 20px;
    overflow: hidden;
  }

  .waveform {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 100%;
    padding: 0 12px;
  }

  .waveform .bar {
    display: inline-block;
    width: 2px;
    background: #7A7468;
    opacity: 0.35;
    border-radius: 1px;
  }

  .hl-span {
    position: absolute;
    top: 0;
    bottom: 0;
  }

  .hl-span.s1 {
    left: 28%;
    width: 7%;
    background: rgba(245, 216, 150, 0.6);
    border-left: 2px solid #E8B96A;
    border-right: 2px solid #E8B96A;
  }

  .hl-span.s2 {
    left: 62%;
    width: 5%;
    background: rgba(200, 212, 181, 0.6);
    border-left: 2px solid #C8D4B5;
    border-right: 2px solid #C8D4B5;
  }

  .hl-span-label {
    position: absolute;
    font-family: 'JetBrains Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #15130F;
    top: -16px;
    white-space: nowrap;
    background: #FFFEFA;
    padding: 0 4px;
  }

  .waveform-timeline {
    display: flex;
    justify-content: space-between;
    padding: 0 12px;
    margin-top: 6px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    color: #7A7468;
  }

  .podcast-highlight {
    padding: 16px 18px;
    background: #EFE9DC;
    border-left: 3px solid #E8B96A;
    margin-bottom: 12px;
    border-radius: 0 2px 2px 0;
  }

  .ph-stamp {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.08em;
    color: #7A7468;
    margin-bottom: 8px;
  }

  .ph-stamp b {
    color: #C24D2C;
    font-weight: 500;
  }

  .ph-quote {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 16px;
    line-height: 1.55;
    color: #15130F;
    margin: 0 0 12px;
  }

  .ph-reaction {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding-top: 10px;
    border-top: 1px dashed #D9D2BF;
    font-size: 13.5px;
    line-height: 1.5;
    color: #3A362E;
  }

  .ph-reaction .member-dot {
    width: 22px;
    height: 22px;
    line-height: 22px;
    font-size: 10px;
    flex-shrink: 0;
  }

  .pr-name {
    font-weight: 500;
    margin-right: 6px;
    color: #15130F;
  }

  /* Room section */
  .room-header-mock {
    margin-top: 36px;
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    border-radius: 4px;
    padding: 28px 32px;
    box-shadow: 0 20px 48px -24px rgba(21, 19, 15, 0.2);
  }

  .room-label {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: #7A7468;
    margin-bottom: 12px;
  }

  .room-name {
    font-family: 'Fraunces', serif;
    font-weight: 500;
    font-size: 32px;
    color: #15130F;
    margin: 0 0 12px;
    letter-spacing: -0.01em;
  }

  .room-name em {
    font-style: italic;
    color: #C24D2C;
    font-weight: 400;
  }

  .room-stats {
    display: flex;
    gap: 24px;
    flex-wrap: wrap;
    font-size: 13.5px;
    color: #7A7468;
    margin-bottom: 20px;
  }

  .room-stats b {
    color: #15130F;
    font-weight: 500;
  }

  .room-activity {
    padding-top: 20px;
    border-top: 1px solid #D9D2BF;
    font-size: 14px;
    line-height: 1.6;
    color: #3A362E;
  }

  .room-activity b {
    color: #15130F;
    font-weight: 500;
  }

  .room-activity + .room-activity {
    border-top: 1px dashed #D9D2BF;
  }

  /* Protocol callout */
  .proto-callout {
    margin-top: 36px;
    padding: 28px 32px;
    background: #FFFEFA;
    border: 1px solid #D9D2BF;
    border-left: 3px solid #C24D2C;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 24px;
    align-items: center;
  }

  @media (max-width: 700px) {
    .proto-callout { grid-template-columns: 1fr; }
  }

  .proto-key {
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-weight: 400;
    font-size: 60px;
    color: #C24D2C;
    line-height: 1;
  }

  .proto-text h5 {
    font-family: 'Fraunces', serif;
    font-weight: 500;
    font-size: 22px;
    margin: 0 0 6px;
    color: #15130F;
  }

  .proto-text p {
    margin: 0;
    font-size: 15px;
    line-height: 1.55;
    color: #3A362E;
    max-width: 52ch;
  }

  .proto-text em { font-style: italic; }

  /* Final CTA */
  .final-cta-buttons {
    display: flex;
    gap: 18px;
    align-items: center;
    margin-top: 28px;
    flex-wrap: wrap;
  }

  .final-fine {
    margin-top: 24px;
    font-family: 'Fraunces', serif;
    font-style: italic;
    font-size: 16px;
    color: #7A7468;
    max-width: 48ch;
  }

</style>
