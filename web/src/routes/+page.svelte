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

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));
  const actions = $derived((signedIn ? memberActions : guestActions) as SurfaceAction[]);

  /* ── Circle membership ── */
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

  const communities = $derived(
    currentUser
      ? buildJoinedCommunities(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
      : []
  );

  /* ── Circle highlight shares ── */
  const circleShareFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': membershipGroupIds, limit: 256 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  let circleHighlights = $state<HydratedHighlight[]>([]);
  let fetchingCircleHighlights = $state(false);

  $effect(() => {
    const shareEvents = [...circleShareFeed.events];
    if (!browser || shareEvents.length === 0) {
      circleHighlights = [];
      fetchingCircleHighlights = false;
      return;
    }

    let cancelled = false;
    fetchingCircleHighlights = true;

    void fetchHighlightsForShares(ndk, shareEvents)
      .then((highlights) => {
        if (!cancelled) circleHighlights = highlights;
      })
      .finally(() => {
        if (!cancelled) fetchingCircleHighlights = false;
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
    for (const hl of circleHighlights) {
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
  const isLoading = $derived(!membershipFeed.eosed || fetchingCircleHighlights);
  const hasCircles = $derived(membershipGroupIds.length > 0);
  const hasFollows = $derived(followPubkeys.length > 0);
  const isEmpty = $derived(!isLoading && mergedHighlights.length === 0);
</script>

{#if signedIn}
  <!-- ═══ SIGNED-IN FEED ═══ -->
  <section class="dashboard-header">
    <div class="dashboard-header-copy">
      <p class="dashboard-eyebrow">YOUR FEED</p>
      <h1 class="dashboard-headline">What your circles are reading</h1>
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
      {:else if isEmpty && !hasCircles && !hasFollows}
        <div class="feed-empty">
          <h2>Your feed starts here.</h2>
          <p>Join a circle or follow someone to see highlights appear in your feed.</p>
          <div class="feed-empty-actions">
            <a href="/community" class="btn-primary">Discover circles</a>
            <a href="/circle/create" class="btn-secondary">Create a circle</a>
          </div>
        </div>
      {:else if isEmpty && hasCircles}
        <div class="feed-empty">
          <h2>Your circles are quiet.</h2>
          <p>No highlights have been shared yet. Be the first.</p>
          <div class="feed-empty-actions">
            <a href="/community" class="btn-primary">Visit your circles</a>
          </div>
        </div>
      {:else if isEmpty && hasFollows}
        <div class="feed-empty">
          <h2>Nothing new from your network.</h2>
          <p>The people you follow haven't shared highlights recently.</p>
          <div class="feed-empty-actions">
            <a href="/community" class="btn-primary">Discover circles</a>
          </div>
        </div>
      {:else}
        <div class="feed-groups">
          {#each visibleGroups as group (group.referenceKey)}
            <HighlightSourceGroup {group} {communities} showShareControl={true} />
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
      {#if communities.length > 0}
        <div class="rail-section">
          <h3 class="rail-heading">Your circles</h3>
          <ul class="rail-circle-list">
            {#each communities.slice(0, 6) as community (community.id)}
              <li>
                <a href="/community/{community.id}" class="rail-circle-link">{community.name}</a>
              </li>
            {/each}
          </ul>
          {#if communities.length > 6}
            <a href="/community" class="rail-view-all">View all circles</a>
          {/if}
        </div>
      {/if}
      <div class="rail-cta-card">
        <h3>Start a new circle</h3>
        <p>Gather your people around the content you care about.</p>
        <a href="/circle/create" class="btn-primary">Create a circle</a>
      </div>
    </aside>
  </section>
{:else}
  <!-- ═══ GUEST VIEW — FULL MARKETING ═══ -->

  <!-- HERO -->
  <section class="hero">
    <div class="hero-inner">
      <div class="hero-eyebrow">Built on Nostr · Your data, your way</div>
      <h1 class="hero-headline">
        Your friends.<br />
        Your highlights.<br />
        <em>Real conversations.</em>
      </h1>
      <p class="hero-sub">
        Private circles for the books, podcasts, and articles you actually care about.
        Share what caught your eye. Watch your circle light up.
        No algorithm. No noise. Just the eight people whose taste you trust.
      </p>
      <div class="hero-ctas">
        <a href="/onboarding" class="btn-primary">Start Your Circle — Free</a>
        <a href="#how" class="btn-ghost">See how it works ↓</a>
      </div>

      <div class="hero-preview">
        <div class="preview-card">
          <div class="preview-card-header">
            <div class="preview-avatar-row">
              <div class="preview-avatar" style="background:#8A9A7F"></div>
              <div class="preview-avatar" style="background:#C47E5E;margin-left:-8px"></div>
              <div class="preview-avatar" style="background:#A36A6A;margin-left:-8px"></div>
              <div class="preview-avatar" style="background:#6B6B6B;margin-left:-8px"></div>
            </div>
            <div class="preview-card-meta">
              <div class="preview-card-name">Non-Fiction 2026</div>
              <div class="preview-card-count">14 members · private</div>
            </div>
          </div>
          <div class="hl-card">
            <div class="hl-label">WHAT CAUGHT OUR EYE</div>
            <p class="hl-text">"The map is not the territory. And yet we keep mistaking the description of reality for reality itself."</p>
            <div class="hl-source">Thinking in Systems · Donella Meadows</div>
          </div>
          <div class="preview-discussion">
            <div class="reply"><span class="reply-name">Mara</span><span class="reply-text">This is the thing I keep failing at with roadmaps…</span></div>
            <div class="reply"><span class="reply-name">James</span><span class="reply-text">Same. Every quarterly plan is technically "a map"</span></div>
            <div class="reply reply-count">+11 more</div>
          </div>
        </div>

        <div class="preview-card preview-card--offset">
          <div class="preview-card-header">
            <div class="preview-avatar-row">
              <div class="preview-avatar" style="background:#C47E5E"></div>
              <div class="preview-avatar" style="background:#8A9A7F;margin-left:-8px"></div>
              <div class="preview-avatar" style="background:#6B6B6B;margin-left:-8px"></div>
            </div>
            <div class="preview-card-meta">
              <div class="preview-card-name">Pod Club</div>
              <div class="preview-card-count">9 members · invite-only</div>
            </div>
          </div>
          <div class="hl-card">
            <div class="hl-label">WHAT CAUGHT OUR EYE</div>
            <p class="hl-text">"The companies that survive recessions aren't the ones that cut the most — they're the ones that cut the right things."</p>
            <div class="hl-source">Acquired · Ep. 182</div>
          </div>
          <div class="preview-discussion">
            <div class="reply"><span class="reply-name">Dev</span><span class="reply-text">Counterintuitive but makes sense in hindsight</span></div>
            <div class="reply reply-count">+5 more</div>
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- PAIN -->
  <section class="pain">
    <div class="pain-inner">
      <div class="pain-label">Sound familiar?</div>
      <h2 class="pain-headline">Every good conversation<br />dies in a group chat</h2>
      <div class="pain-quotes">
        <blockquote>
          "I want a semi-private space to share links and discuss — but Slack feels like too much and a group chat feels too thin."
        </blockquote>
        <blockquote>
          "The thing that's frustrating about podcasts is that they're not easily shared and discussed."
        </blockquote>
        <blockquote>
          "Book clubs tend to feel like an assignment. I far prefer talking with a few book friends."
        </blockquote>
      </div>
      <p class="pain-closer">
        WhatsApp buries threads. Discord needs a server. Goodreads is a review site. Readwise stays solo.
        <strong>No one built the thing for small, trusted, content-centric circles.</strong> Until now.
      </p>
    </div>
  </section>

  <!-- HOW IT WORKS -->
  <section class="how" id="how">
    <div class="how-inner">
      <div class="section-label">Simple by design</div>
      <h2 class="section-headline">How it works</h2>

      <div class="steps">
        <div class="step">
          <div class="step-number">01</div>
          <div class="step-content">
            <h3>Create your circle</h3>
            <p>Start a circle for your friend group, book club, or podcast crew. Choose who can join and who can see it. Invite with a link.</p>
          </div>
        </div>
        <div class="step-connector"></div>
        <div class="step">
          <div class="step-number">02</div>
          <div class="step-content">
            <h3>Share what you're into</h3>
            <p>Drop any book, article, podcast episode, or video. Highlighter extracts what's relevant automatically — or paste a passage yourself.</p>
          </div>
        </div>
        <div class="step-connector"></div>
        <div class="step">
          <div class="step-number">03</div>
          <div class="step-content">
            <h3>Watch highlights spark discussions</h3>
            <p>Pull out the sentence that made you stop. Your circle responds. The conversation is organized around the content — not lost in a thread.</p>
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- YOUR CIRCLES -->
  <section class="circles">
    <div class="circles-inner">
      <div class="section-label">Who it's for</div>
      <h2 class="section-headline">Your circles, your rules</h2>

      <div class="circle-cards">
        <div class="circle-card">
          <div class="circle-emoji">📚</div>
          <h3>The book club that actually talks</h3>
          <p>Share chapters. Highlight moments. Have the conversation asynchronously, on everyone's schedule — without the logistics of a monthly meeting.</p>
          <div class="circle-tag">8–20 members</div>
        </div>
        <div class="circle-card circle-card--accent">
          <div class="circle-emoji">🎙️</div>
          <h3>The podcast circle</h3>
          <p>Finally — a home for your "We need to talk about that episode" messages. Share episodes, clip the moment, discuss in depth.</p>
          <div class="circle-tag">Great for 5–15 friends</div>
        </div>
        <div class="circle-card">
          <div class="circle-emoji">🧠</div>
          <h3>The founder/thinker crew</h3>
          <p>Share the article that changed your thinking. Highlight the one paragraph that matters. Skip the noise of group chats.</p>
          <div class="circle-tag">Tight, smart, trusted</div>
        </div>
        <div class="circle-card">
          <div class="circle-emoji">🏠</div>
          <h3>The family "what we're learning" group</h3>
          <p>A private space to share what you're reading and watching with the people you love. High trust, zero pressure, completely private.</p>
          <div class="circle-tag">Invite-only by default</div>
        </div>
      </div>
    </div>
  </section>

  <!-- FEATURES -->
  <section class="features">
    <div class="features-inner">
      <div class="section-label">What makes it different</div>
      <h2 class="section-headline">Every word is organized<br />around what you're reading</h2>

      <div class="feature-grid">
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>The artifact is the hero</h4>
          <p>Every conversation lives under the book, episode, or article — not lost in a timeline. Come back to it in a week. Still there, still organized.</p>
        </div>
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>Highlights, not hot takes</h4>
          <p>The atomic unit is an excerpt from something real — not a hot take from nothing. Conversations start grounded in the actual content.</p>
        </div>
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>Private by default</h4>
          <p>Your circle's conversations belong to your circle. Nothing is public unless you choose. No one can accidentally see your discussion.</p>
        </div>
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>Share one thing to multiple circles</h4>
          <p>Post the same article to your book club and your work crew simultaneously. Each circle has its own conversation. Your vault collects everything.</p>
        </div>
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>Browser extension + mobile capture</h4>
          <p>One click from any webpage. Snap a Kindle highlight. Share a podcast moment. Getting things into Highlighter is faster than texting a link.</p>
        </div>
        <div class="feature">
          <div class="feature-icon">✦</div>
          <h4>Your personal vault</h4>
          <p>Every highlight you've ever made, searchable and yours forever — across all your circles. Never lose something you marked as worth keeping.</p>
        </div>
      </div>
    </div>
  </section>

  <!-- NOSTR -->
  <section class="nostr">
    <div class="nostr-inner">
      <div class="nostr-text">
        <div class="section-label">Built differently</div>
        <h2>You own your circles.<br />We just host them.</h2>
        <p>Highlighter is built on <strong>Nostr</strong> — an open protocol where you control your identity and your data. No email address required. No lock-in. Your circles and highlights are standard events on an open network.</p>
        <p>If we ever shut down (we won't, but: <em>if</em>), your data goes with you. Your key is your identity.</p>
        <div class="nostr-pills">
          <span class="pill">Own your identity</span>
          <span class="pill">Portable data</span>
          <span class="pill">No platform lock-in</span>
          <span class="pill">Open protocol</span>
        </div>
      </div>
      <div class="nostr-visual">
        <div class="key-card">
          <div class="key-card-label">Your identity</div>
          <div class="key-display">
            <span class="key-prefix">npub</span><span class="key-chars">1qzf…k94m</span>
          </div>
          <div class="key-card-sub">Works on every Nostr app. Forever yours.</div>
        </div>
        <div class="relay-visual-card">
          <div class="relay-row">
            <div class="relay-dot relay-dot--green"></div>
            <span>highlighter.xyz relay</span>
          </div>
          <div class="relay-row">
            <div class="relay-dot relay-dot--green"></div>
            <span>nostr.wine</span>
          </div>
          <div class="relay-row">
            <div class="relay-dot relay-dot--amber"></div>
            <span>your-own-relay.io</span>
          </div>
          <div class="relay-card-note">Run on your relay, or ours.</div>
        </div>
      </div>
    </div>
  </section>

  <!-- SOCIAL PROOF -->
  <section class="proof">
    <div class="proof-inner">
      <div class="proof-label">From real people, real frustration</div>
      <div class="proof-quotes">
        <div class="proof-quote">
          <p>"Not having friends with similar interests as you suuuuuucks. Who else am I supposed to discuss podcasts with?"</p>
          <span>— X user</span>
        </div>
        <div class="proof-quote">
          <p>"Readwise but social private community for highlights."</p>
          <span>— X user (this is exactly what we built)</span>
        </div>
        <div class="proof-quote">
          <p>"Move over, book clubs! The PodClub is the new way…"</p>
          <span>— The Guardian, May 2025</span>
        </div>
      </div>
    </div>
  </section>

  <!-- FINAL CTA -->
  <section class="final-cta">
    <div class="final-cta-inner">
      <h2>Ready for high-signal conversations again?</h2>
      <p>Invite your 8 friends. Share the first highlight. See what happens.</p>
      <a href="/onboarding" class="btn-primary btn-large">Start Your Circle — Free</a>
      <div class="cta-footnote">No credit card. No email address required. Just your Nostr key.</div>
    </div>
  </section>
{/if}

<style>
  /* Reset the .page gap/padding for the guest marketing flow */
  :global(.page:has(.hero)) {
    gap: 0;
    padding: 0;
  }

  /* ═══ SHARED ═══ */

  .section-label {
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    margin-bottom: 1rem;
  }

  .section-headline {
    font-family: var(--font-serif);
    font-size: clamp(1.75rem, 4vw, 2.75rem);
    font-weight: 700;
    line-height: 1.2;
    color: var(--text-strong);
    margin-bottom: 3rem;
  }

  /* ═══ SIGNED-IN FEED ═══ */

  .dashboard-header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.5rem 0 0;
  }

  .dashboard-eyebrow {
    margin: 0;
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--accent);
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

  .feed-main {
    min-width: 0;
  }

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

  .feed-loading-text,
  .feed-resolving {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .feed-resolving {
    margin-top: 0.75rem;
  }

  /* Skeleton loading */
  .feed-skeleton {
    display: grid;
    gap: 0.9rem;
  }

  .skeleton-card {
    height: 10rem;
    border-radius: 1rem;
    background: linear-gradient(
      110deg,
      var(--surface-soft) 30%,
      var(--surface) 50%,
      var(--surface-soft) 70%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
  }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* Empty states */
  .feed-empty {
    display: grid;
    gap: 0.5rem;
    padding: 2.5rem 2rem;
    border: 1px solid var(--border);
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

  /* Sidebar rail */
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

  .rail-circle-link:hover {
    background: var(--surface-soft);
  }

  .rail-view-all {
    font-size: 0.813rem;
    color: var(--accent);
    text-decoration: none;
    padding-left: 0.625rem;
  }

  .rail-view-all:hover {
    text-decoration: underline;
  }

  .rail-cta-card {
    display: grid;
    gap: 0.5rem;
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface);
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

  /* ═══ BUTTONS ═══ */

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
    border: 1px solid var(--border);
    transition: box-shadow 0.18s ease, transform 0.12s ease;
    cursor: pointer;
    white-space: nowrap;
    text-decoration: none;
  }

  .btn-secondary:hover {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
    transform: translateY(-1px);
  }

  .btn-ghost {
    display: inline-block;
    color: rgba(248, 245, 240, 0.55);
    font-size: 0.94rem;
    font-weight: 400;
    padding: 0.85rem 0.25rem;
    border-bottom: 1px solid transparent;
    transition: color 0.15s, border-color 0.15s;
    text-decoration: none;
  }

  .btn-ghost:hover {
    color: #F8F5F0;
    border-bottom-color: #F8F5F0;
  }

  .btn-large {
    font-size: 1.05rem;
    padding: 1.1rem 2.5rem;
  }

  /* ═══ HERO ═══ */

  .hero {
    background: #1F1F1F;
    color: #F8F5F0;
    padding: 5rem 0 0;
    overflow: hidden;
    margin: -3rem calc(-1 * (100vw - min(calc(100vw - 2.5rem), var(--page-width))) / 2) 0;
    width: 100vw;
    position: relative;
    left: 50%;
    transform: translateX(-50%);
  }

  .hero-inner {
    max-width: var(--page-width);
    margin: 0 auto;
    padding: 0 1.5rem;
    text-align: center;
  }

  .hero-eyebrow {
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    margin-bottom: 1.5rem;
  }

  .hero-headline {
    font-family: var(--font-serif);
    font-size: clamp(2.5rem, 7vw, 4.75rem);
    font-weight: 700;
    line-height: 1.1;
    letter-spacing: -0.02em;
    margin-bottom: 1.75rem;
    color: #F8F5F0;
  }

  .hero-headline em {
    font-style: italic;
    color: var(--accent);
  }

  .hero-sub {
    font-size: clamp(1rem, 2vw, 1.25rem);
    line-height: 1.6;
    color: rgba(248, 245, 240, 0.7);
    max-width: 40rem;
    margin: 0 auto 2.5rem;
  }

  .hero-ctas {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1.5rem;
    margin-bottom: 4.5rem;
    flex-wrap: wrap;
  }

  /* Hero preview cards */
  .hero-preview {
    display: flex;
    gap: 1.25rem;
    justify-content: center;
    align-items: flex-start;
    padding-bottom: 0;
  }

  .preview-card {
    background: #2A2A2A;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.75rem;
    padding: 1.25rem;
    width: 20rem;
    flex-shrink: 0;
    text-align: left;
  }

  .preview-card--offset {
    position: relative;
    top: 2.5rem;
  }

  .preview-card-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
  }

  .preview-avatar-row {
    display: flex;
  }

  .preview-avatar {
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 50%;
    border: 2px solid #2A2A2A;
    flex-shrink: 0;
  }

  .preview-card-name {
    font-size: 0.875rem;
    font-weight: 500;
    color: #F8F5F0;
  }

  .preview-card-count {
    font-size: 0.75rem;
    color: rgba(248, 245, 240, 0.45);
    margin-top: 0.125rem;
  }

  .hl-card {
    background: rgba(196, 126, 94, 0.1);
    border-left: 2.5px solid var(--accent);
    padding: 0.875rem 1rem;
    border-radius: 0 0.5rem 0.5rem 0;
    margin-bottom: 0.875rem;
  }

  .hl-label {
    font-size: 0.625rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    margin-bottom: 0.5rem;
  }

  .hl-text {
    font-size: 0.875rem;
    line-height: 1.55;
    color: rgba(248, 245, 240, 0.88);
    font-style: italic;
    margin-bottom: 0.5rem;
  }

  .hl-source {
    font-size: 0.688rem;
    color: rgba(248, 245, 240, 0.4);
  }

  .preview-discussion {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .reply {
    font-size: 0.813rem;
    color: rgba(248, 245, 240, 0.65);
  }

  .reply-name {
    font-weight: 500;
    color: rgba(248, 245, 240, 0.85);
    margin-right: 0.375rem;
  }

  .reply-count {
    color: rgba(248, 245, 240, 0.35);
    font-size: 0.75rem;
  }

  /* ═══ PAIN ═══ */

  .pain {
    padding: 6rem 0;
    text-align: center;
  }

  .pain-inner {
    max-width: var(--page-width);
    margin: 0 auto;
  }

  .pain-label {
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
    margin-bottom: 1rem;
  }

  .pain-headline {
    font-family: var(--font-serif);
    font-size: clamp(1.75rem, 4vw, 3rem);
    font-weight: 700;
    line-height: 1.15;
    margin-bottom: 3rem;
    letter-spacing: -0.01em;
    color: var(--text-strong);
  }

  .pain-quotes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: 1.5rem;
    max-width: 58rem;
    margin: 0 auto 3rem;
    text-align: left;
  }

  .pain-quotes blockquote {
    background: var(--surface-soft);
    border-radius: 0.75rem;
    padding: 1.5rem;
    font-size: 0.94rem;
    line-height: 1.6;
    color: var(--text);
    font-style: italic;
    position: relative;
    margin: 0;
  }

  .pain-quotes blockquote::before {
    content: '\201C';
    font-family: var(--font-serif);
    font-size: 3rem;
    color: var(--accent);
    opacity: 0.4;
    position: absolute;
    top: 0.5rem;
    left: 1rem;
    line-height: 1;
  }

  .pain-closer {
    font-size: 1.1rem;
    line-height: 1.6;
    color: var(--muted);
    max-width: 36rem;
    margin: 0 auto;
  }

  .pain-closer strong {
    color: var(--text-strong);
  }

  /* ═══ HOW IT WORKS ═══ */

  .how {
    background: var(--surface-soft);
    padding: 6rem 0;
    margin: 0 calc(-1 * (100vw - min(calc(100vw - 2.5rem), var(--page-width))) / 2);
    width: 100vw;
    position: relative;
    left: 50%;
    transform: translateX(-50%);
  }

  .how-inner {
    max-width: var(--page-width);
    margin: 0 auto;
    padding: 0 1.5rem;
  }

  .steps {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .step {
    flex: 1;
    background: var(--surface);
    border-radius: 0.75rem;
    padding: 2.25rem 2rem;
  }

  .step-connector {
    width: 2.5rem;
    height: 2px;
    background: var(--accent);
    opacity: 0.3;
    flex-shrink: 0;
  }

  .step-number {
    font-size: 0.688rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: var(--accent);
    margin-bottom: 1rem;
  }

  .step h3 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.625rem;
    line-height: 1.3;
    color: var(--text-strong);
  }

  .step p {
    font-size: 0.94rem;
    color: var(--muted);
    line-height: 1.6;
    margin: 0;
  }

  /* ═══ CIRCLES ═══ */

  .circles {
    padding: 6rem 0;
  }

  .circles-inner {
    max-width: var(--page-width);
    margin: 0 auto;
  }

  .circle-cards {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 1.25rem;
  }

  .circle-card {
    background: var(--surface-soft);
    border-radius: 0.75rem;
    padding: 2.25rem 2rem;
    position: relative;
    overflow: hidden;
  }

  .circle-card--accent {
    background: #1F1F1F;
    color: #F8F5F0;
  }

  .circle-card--accent h3 {
    color: #F8F5F0;
  }

  .circle-card--accent p {
    color: rgba(248, 245, 240, 0.65);
  }

  .circle-emoji {
    font-size: 2rem;
    margin-bottom: 1.25rem;
    display: block;
  }

  .circle-card h3 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    line-height: 1.3;
    color: var(--text-strong);
  }

  .circle-card p {
    font-size: 0.94rem;
    color: var(--muted);
    line-height: 1.6;
    margin-bottom: 1.25rem;
  }

  .circle-tag {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--accent);
    letter-spacing: 0.04em;
    padding: 0.375rem 0.75rem;
    background: rgba(196, 126, 94, 0.1);
    border-radius: 9999px;
    display: inline-block;
  }

  .circle-card--accent .circle-tag {
    background: rgba(196, 126, 94, 0.15);
  }

  /* ═══ FEATURES ═══ */

  .features {
    background: var(--surface-soft);
    padding: 6rem 0;
    margin: 0 calc(-1 * (100vw - min(calc(100vw - 2.5rem), var(--page-width))) / 2);
    width: 100vw;
    position: relative;
    left: 50%;
    transform: translateX(-50%);
  }

  .features-inner {
    max-width: var(--page-width);
    margin: 0 auto;
    padding: 0 1.5rem;
  }

  .feature-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    overflow: hidden;
  }

  .feature {
    padding: 2.25rem 2rem;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .feature:nth-child(3n) {
    border-right: none;
  }

  .feature:nth-child(n + 4) {
    border-bottom: none;
  }

  .feature-icon {
    color: var(--accent);
    font-size: 1.125rem;
    margin-bottom: 1rem;
  }

  .feature h4 {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 0.625rem;
    line-height: 1.3;
    color: var(--text-strong);
  }

  .feature p {
    font-size: 0.875rem;
    color: var(--muted);
    line-height: 1.6;
    margin: 0;
  }

  /* ═══ NOSTR ═══ */

  .nostr {
    padding: 6rem 0;
  }

  .nostr-inner {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4rem;
    align-items: center;
    max-width: var(--page-width);
    margin: 0 auto;
  }

  .nostr-text h2 {
    font-family: var(--font-serif);
    font-size: clamp(1.625rem, 3.5vw, 2.5rem);
    font-weight: 700;
    line-height: 1.2;
    margin-bottom: 1.5rem;
    color: var(--text-strong);
  }

  .nostr-text p {
    font-size: 1rem;
    color: var(--muted);
    line-height: 1.7;
    margin-bottom: 1rem;
  }

  .nostr-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 1.75rem;
  }

  .pill {
    font-size: 0.813rem;
    font-weight: 500;
    padding: 0.375rem 0.875rem;
    background: var(--surface-soft);
    border-radius: 9999px;
    color: var(--text);
  }

  .nostr-visual {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .key-card {
    background: #1F1F1F;
    border-radius: 0.75rem;
    padding: 1.5rem 1.75rem;
    color: #F8F5F0;
  }

  .key-card-label {
    font-size: 0.688rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(248, 245, 240, 0.4);
    margin-bottom: 0.625rem;
  }

  .key-display {
    font-family: var(--font-mono);
    font-size: 1.125rem;
    margin-bottom: 0.5rem;
  }

  .key-prefix {
    color: var(--accent);
    margin-right: 0.125rem;
  }

  .key-chars {
    color: rgba(248, 245, 240, 0.8);
  }

  .key-card-sub {
    font-size: 0.813rem;
    color: rgba(248, 245, 240, 0.4);
  }

  .relay-visual-card {
    background: var(--surface-soft);
    border-radius: 0.75rem;
    padding: 1.25rem 1.5rem;
  }

  .relay-row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.5rem 0;
    font-size: 0.875rem;
    color: var(--text);
    border-bottom: 1px solid var(--border);
  }

  .relay-row:last-of-type {
    border-bottom: none;
  }

  .relay-dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .relay-dot--green {
    background: #8A9A7F;
  }

  .relay-dot--amber {
    background: #C9A84C;
  }

  .relay-card-note {
    font-size: 0.75rem;
    color: var(--muted);
    margin-top: 0.75rem;
  }

  /* ═══ SOCIAL PROOF ═══ */

  .proof {
    background: #1F1F1F;
    padding: 6rem 0;
    color: #F8F5F0;
    margin: 0 calc(-1 * (100vw - min(calc(100vw - 2.5rem), var(--page-width))) / 2);
    width: 100vw;
    position: relative;
    left: 50%;
    transform: translateX(-50%);
  }

  .proof-inner {
    max-width: var(--page-width);
    margin: 0 auto;
    padding: 0 1.5rem;
  }

  .proof-label {
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(248, 245, 240, 0.4);
    margin-bottom: 3rem;
    text-align: center;
  }

  .proof-quotes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 2rem;
  }

  .proof-quote {
    padding: 1.75rem 0;
    border-top: 1px solid rgba(248, 245, 240, 0.12);
  }

  .proof-quote p {
    font-size: 1rem;
    line-height: 1.6;
    color: rgba(248, 245, 240, 0.85);
    font-style: italic;
    margin-bottom: 1rem;
  }

  .proof-quote span {
    font-size: 0.813rem;
    color: rgba(248, 245, 240, 0.35);
  }

  /* ═══ FINAL CTA ═══ */

  .final-cta {
    padding: 6rem 0;
    text-align: center;
  }

  .final-cta-inner {
    max-width: var(--page-width);
    margin: 0 auto;
  }

  .final-cta h2 {
    font-family: var(--font-serif);
    font-size: clamp(1.75rem, 4vw, 3rem);
    font-weight: 700;
    margin-bottom: 1rem;
    letter-spacing: -0.01em;
    color: var(--text-strong);
  }

  .final-cta p {
    font-size: 1.1rem;
    color: var(--muted);
    margin-bottom: 2.5rem;
  }

  .cta-footnote {
    margin-top: 1.25rem;
    font-size: 0.813rem;
    color: var(--muted);
  }

  /* ═══ RESPONSIVE ═══ */

  @media (max-width: 820px) {
    .dashboard-header {
      flex-direction: column;
      align-items: flex-start;
    }

    .dashboard-body {
      grid-template-columns: 1fr;
    }

    .feed-rail {
      order: 1;
    }
  }

  @media (max-width: 600px) {
    .dashboard-ctas {
      width: 100%;
    }

    .dashboard-ctas .btn-primary,
    .dashboard-ctas .btn-secondary {
      flex: 1;
      text-align: center;
    }

    .rail-cta-card {
      text-align: center;
    }
  }

  @media (max-width: 900px) {
    .hero-preview {
      display: none;
    }

    .steps {
      flex-direction: column;
      gap: 1.25rem;
    }

    .step-connector {
      width: 2px;
      height: 1.5rem;
    }

    .circle-cards {
      grid-template-columns: 1fr;
    }

    .feature-grid {
      grid-template-columns: repeat(2, 1fr);
    }

    .feature:nth-child(3n) {
      border-right: 1px solid var(--border);
    }

    .feature:nth-child(2n) {
      border-right: none;
    }

    .feature:nth-last-child(-n + 2) {
      border-bottom: none;
    }

    .nostr-inner {
      grid-template-columns: 1fr;
      gap: 2.5rem;
    }

    .proof-quotes {
      grid-template-columns: 1fr;
      gap: 0;
    }
  }

  @media (max-width: 600px) {
    .hero {
      padding: 3.75rem 0 0;
    }

    .hero-ctas {
      flex-direction: column;
      gap: 0.75rem;
    }

    .feature-grid {
      grid-template-columns: 1fr;
    }

    .feature {
      border-right: none;
    }

    .feature:not(:last-child) {
      border-bottom: 1px solid var(--border);
    }

    .pain-quotes {
      grid-template-columns: 1fr;
    }

    .how,
    .features,
    .proof {
      padding: 3.5rem 0;
    }

    .pain,
    .circles,
    .nostr,
    .final-cta {
      padding: 3.5rem 0;
    }
  }
</style>
