<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import MemberStack from './MemberStack.svelte';
  import PodcastPlayer from './PodcastPlayer.svelte';
  import TimelineStamp from './TimelineStamp.svelte';
  import { memberTint } from '../utils/colors';

  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  interface ArtifactCardProps {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
    highlightCount?: number;
    discussionCount?: number;
  }

  interface Member {
    pubkey: string;
    colorIndex: number;
    name: string;
    joinedAt?: string;
  }

  let {
    artifact,
    onBack,
    members
  }: {
    artifact: ArtifactCardProps;
    onBack: () => void;
    members: Member[];
  } = $props();

  const seedHighlightSpans = $derived([
    { start: '12:34', end: '13:15', memberIdx: 0 },
    { start: '28:15', end: '29:00', memberIdx: 1 },
    { start: '45:02', end: '46:30', memberIdx: 2 },
    { start: '1:02:30', end: '1:03:45', memberIdx: 4 },
    { start: '1:18:45', end: '1:20:00', memberIdx: 3 },
    { start: '1:34:20', end: '1:35:30', memberIdx: 5 }
  ].flatMap((s) => {
    const m = members[s.memberIdx];
    if (!m) return [];
    return [{ start: s.start, end: s.end, colorIndex: m.colorIndex, memberName: m.name }];
  }));

  const seedChapters = [
    { time: '00:00', title: 'Introduction — The Sovereign Individual Framework' },
    { time: '12:34', title: 'The Death of Distance' },
    { time: '28:15', title: 'Digital Governance & Sovereignty' },
    { time: '45:02', title: 'Antifragility in the Information Age' },
    { time: '1:02:30', title: 'Economic Transition Analysis' },
    { time: '1:18:45', title: 'Technical Architecture & Resilience' }
  ];

  const seedTimeline = [
    { timestamp: '12:34', memberIdx: 0, note: 'The sovereign individual framework here is core to the whole argument.' },
    { timestamp: '28:15', memberIdx: 1, note: 'This point about digital governance predates the internet by a decade.' },
    { timestamp: '45:02', memberIdx: 2, note: "Parallel to Taleb's antifragility concept." },
    { timestamp: '1:02:30', memberIdx: 4, note: 'The economic transition analysis is still accurate.' },
    { timestamp: '1:18:45', memberIdx: 3, note: 'Technical architecture analogy: sovereign = resilient system.' },
    { timestamp: '1:34:20', memberIdx: 5, note: 'This historical framing is exactly right.' }
  ];

  const seedMemberProgress = [
    { memberIdx: 0, progress: 87 },
    { memberIdx: 1, progress: 64 },
    { memberIdx: 2, progress: 45 },
    { memberIdx: 3, progress: 92 },
    { memberIdx: 4, progress: 73 },
    { memberIdx: 5, progress: 31 }
  ];

  function memberAt(idx: number): Member | undefined {
    return members[idx];
  }

  let activeChapter = $state(0);

  function handleChapterClick(index: number) {
    activeChapter = index;
    console.log('seek to chapter:', seedChapters[index].time, seedChapters[index].title);
  }
</script>

<div class="podcast-view">
  <!-- Back button -->
  <div class="podcast-nav">
    <button class="back-btn" type="button" onclick={onBack}>
      ← Back to room
    </button>
  </div>

  <!-- Hero -->
  <header class="podcast-hero">
    <div class="hero-cover-wrap">
      {#if artifact.cover}
        <img class="hero-cover" src={artifact.cover} alt="" width="200" height="200" />
      {:else}
        <div class="hero-cover hero-cover-placeholder">🎙</div>
      {/if}
      <button class="play-overlay" type="button" aria-label="Play podcast">
        <span aria-hidden="true">▶</span>
      </button>
    </div>

    <div class="hero-meta">
      <span class="podcast-kicker">PODCAST</span>
      <h1 class="podcast-title">{artifact.title}</h1>
      {#if artifact.author}
        <p class="podcast-author">{artifact.author}</p>
      {/if}
      <div class="members-strip">
        <MemberStack {members} />
        <span class="members-label">{members.length} members listening</span>
      </div>
    </div>
  </header>

  <!-- Player -->
  <div class="player-section">
    <PodcastPlayer
      duration="1:45:20"
      currentTime="0:00"
      highlightSpans={seedHighlightSpans}
    />
  </div>

  <!-- Body: Timeline + Chapters/Progress -->
  <div class="podcast-body">
    <!-- Timeline (left 60%) -->
    <section class="timeline-section">
      <h2 class="section-title">Member Timestamps</h2>
      <div class="timeline-list">
        {#each seedTimeline as stamp, i (i)}
          {@const m = memberAt(stamp.memberIdx)}
          {#if m}
            <TimelineStamp
              timestamp={stamp.timestamp}
              pubkey={m.pubkey}
              memberColorIndex={m.colorIndex}
              note={stamp.note}
            />
          {/if}
        {/each}
      </div>
    </section>

    <!-- Chapters + Progress (right 40%) -->
    <div class="podcast-sidebar">
      <!-- Chapters -->
      <section class="chapters-section">
        <h2 class="section-title">Chapters</h2>
        <ul class="chapters-list" role="list">
          {#each seedChapters as chapter, i (i)}
            <li class="chapter-item" class:active={activeChapter === i}>
              <button
                class="chapter-btn"
                type="button"
                onclick={() => handleChapterClick(i)}
              >
                <span class="chapter-time">{chapter.time}</span>
                <span class="chapter-title">{chapter.title}</span>
              </button>
            </li>
          {/each}
        </ul>
      </section>

      <!-- Member Progress -->
      <section class="progress-section">
        <h2 class="section-title">Listening Progress</h2>
        <div class="progress-grid">
          {#each seedMemberProgress as mp (mp.memberIdx)}
            {@const m = memberAt(mp.memberIdx)}
            {#if m}
              {@const color = memberTint(m.colorIndex)}
              <User.Root {ndk} pubkey={m.pubkey}>
                <div class="progress-item">
                  <span
                    class="room-member-avatar"
                    style:--mav-size="24px"
                    style:--mav-ring={color}
                    style:--mav-ring-width="1.5px"
                  >
                    <User.Avatar />
                  </span>
                  <div class="progress-info">
                    <span class="progress-name"><User.Name field="displayName" /></span>
                    <div class="progress-bar-track">
                      <div
                        class="progress-bar-fill"
                        style:width="{mp.progress}%"
                        style:background={color}
                      ></div>
                    </div>
                    <span class="progress-pct">{mp.progress}%</span>
                  </div>
                </div>
              </User.Root>
            {/if}
          {/each}
        </div>
      </section>
    </div>
  </div>
</div>

<style>
  .podcast-view {
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding-top: 24px;
    padding-bottom: 80px;
    padding-left: var(--container-px, 40px);
    padding-right: var(--container-px, 40px);
    max-width: var(--container-max, 1440px);
    margin: 0 auto;
  }

  .back-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink-soft);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    transition: color var(--transition);
  }

  .back-btn:hover {
    color: var(--brand-accent);
  }

  .back-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }

  /* Hero */
  .podcast-hero {
    display: flex;
    flex-direction: column;
    gap: 32px;
    align-items: flex-start;
  }

  .hero-cover-wrap {
    position: relative;
    flex-shrink: 0;
  }

  .hero-cover {
    width: 160px;
    height: 160px;
    border-radius: var(--radius);
    object-fit: cover;
    display: block;
  }

  .hero-cover-placeholder {
    background-color: var(--surface-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 48px;
    border: 1px solid var(--rule);
  }

  .play-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--overlay-dark);
    border: none;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 32px;
    color: var(--surface);
    opacity: 0;
    transition: opacity var(--transition);
  }

  .hero-cover-wrap:hover .play-overlay {
    opacity: 1;
  }

  .hero-meta {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding-top: 4px;
  }

  .podcast-kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .podcast-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(28px, 4vw, 48px);
    color: var(--ink);
    line-height: 1.15;
    margin: 0;
  }

  .podcast-author {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 400;
    color: var(--ink-soft);
    margin: 0;
  }

  .members-strip {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .members-label {
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ink-fade);
  }

  /* Body grid */
  .podcast-body {
    display: grid;
    grid-template-columns: 1fr;
    gap: 40px;
    align-items: start;
  }

  .section-title {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0 0 14px;
  }

  /* Timeline */
  .timeline-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* Sidebar */
  .podcast-sidebar {
    display: flex;
    flex-direction: column;
    gap: 28px;
    position: static;
  }

  /* Chapters */
  .chapters-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .chapter-item {
    border-bottom: 1px solid var(--rule-soft);
  }

  .chapter-item:last-child {
    border-bottom: none;
  }

  .chapter-btn {
    display: flex;
    gap: 12px;
    align-items: baseline;
    width: 100%;
    padding: 10px 0;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: color var(--transition);
  }

  .chapter-btn:hover .chapter-title {
    color: var(--brand-accent);
  }

  .chapter-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }

  .play-overlay:focus-visible {
    outline: 2px solid var(--surface);
    outline-offset: -4px;
    opacity: 1;
  }

  .chapter-item.active .chapter-time,
  .chapter-item.active .chapter-title {
    color: var(--brand-accent);
  }

  .chapter-time {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    flex-shrink: 0;
    min-width: 44px;
  }

  .chapter-item.active .chapter-time {
    color: var(--brand-accent);
  }

  .chapter-title {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    line-height: 1.4;
  }

  /* Progress */
  .progress-grid {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .progress-item {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .progress-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .progress-name {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .progress-bar-track {
    height: 4px;
    background-color: var(--surface-muted);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    border-radius: 2px;
    transition: width var(--transition);
  }

  .progress-pct {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    align-self: flex-end;
  }

  @media (min-width: 768px) {
    .podcast-hero {
      flex-direction: row;
    }

    .hero-cover {
      width: 200px;
      height: 200px;
    }

    .podcast-body {
      grid-template-columns: 3fr 2fr;
    }

    .podcast-sidebar {
      position: sticky;
      top: 24px;
    }
  }
</style>
