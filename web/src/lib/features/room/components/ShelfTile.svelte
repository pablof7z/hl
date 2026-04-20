<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  type MediaType = 'book' | 'podcast' | 'essay' | 'paper' | 'archive';
  type BookVariant = 'dark' | 'red' | 'blue' | 'green' | 'plum';
  type PodcastVariant = 'default' | 'orange';
  type Status = 'reading' | 'this-week' | 're-read' | 'none';

  interface Engager {
    pubkey: string;
    colorIndex: number;
  }

  let {
    id,
    type,
    title,
    author,
    typeChipLabel,
    bookVariant = 'dark',
    podcastVariant = 'default',
    status = 'none',
    statusLabel,
    engaged = [],
    stats,
    href = '#'
  }: {
    id: string;
    type: MediaType;
    title: string;
    author?: string;
    typeChipLabel?: string;
    bookVariant?: BookVariant;
    podcastVariant?: PodcastVariant;
    status?: Status;
    statusLabel?: string;
    engaged?: Engager[];
    stats?: string;
    href?: string;
  } = $props();

  const coverClasses = $derived(() => {
    const classes = ['shelf-cover', type];
    if (type === 'book') classes.push(bookVariant);
    if (type === 'podcast' && podcastVariant === 'orange') classes.push('orange');
    return classes.join(' ');
  });

  const statusClass = $derived(() => {
    if (status === 'reading') return 'open';
    if (status === 'this-week' || status === 're-read') return 'week';
    return '';
  });
</script>

<a {href} class="shelf-tile" data-id={id}>
  <div class={coverClasses()}>
    {#if status !== 'none'}
      <div class="shelf-status {statusClass()}">{statusLabel ?? status}</div>
    {/if}

    {#if typeChipLabel}
      <div class="type-chip">{typeChipLabel}</div>
    {/if}

    {#if type === 'essay'}
      <div class="essay-mark">§</div>
    {/if}

    {#if type === 'paper'}
      <div class="paper-lines"></div>
    {/if}

    {#if type === 'archive'}
      <div class="archive-mark">"</div>
    {/if}

    <div class="cover-content">
      <div class="sc-title">{title}</div>
      {#if author}<div class="sc-author">{author}</div>{/if}
    </div>
  </div>

  <div class="shelf-meta">
    <div class="shelf-tile-title">{title}</div>
    {#if author}<div class="shelf-tile-author">{author}</div>{/if}
  </div>

  <div class="shelf-tile-foot">
    <div class="dots">
      {#each engaged as member, i (member.pubkey)}
        <span class:overlap={i > 0}>
          <User.Root {ndk} pubkey={member.pubkey}>
            <span
              class="room-member-avatar"
              style:--mav-size="18px"
              style:--mav-ring={memberTint(member.colorIndex)}
              style:--mav-ring-width="1.5px"
            >
              <User.Avatar />
            </span>
          </User.Root>
        </span>
      {/each}
    </div>
    {#if stats}<span>{stats}</span>{/if}
  </div>
</a>

<style>
  .shelf-tile {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    text-decoration: none;
    color: inherit;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: border-color 200ms, transform 200ms;
  }

  .shelf-tile:hover {
    border-color: var(--brand-accent);
    transform: translateY(-2px);
  }

  .shelf-cover {
    aspect-ratio: 4/5;
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 14px;
    color: inherit;
    overflow: hidden;
  }

  .type-chip {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    padding: 2px 6px;
    align-self: flex-start;
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.2);
    color: currentColor;
    font-weight: 500;
    z-index: 2;
  }

  .cover-content {
    z-index: 2;
    margin-top: auto;
  }

  .sc-title {
    white-space: pre-line;
  }

  /* ── Book covers ───────────────────────────── */
  .shelf-cover.book {
    background: linear-gradient(140deg, #1C1814 0%, #3A2D1E 55%, #1C1814 100%);
    color: #E6D9BC;
    font-family: var(--font-serif);
  }
  .shelf-cover.book.blue { background: linear-gradient(140deg, #141C2E 0%, #1C2D4D 55%, #141C2E 100%); color: #D6DBE6; }
  .shelf-cover.book.red { background: linear-gradient(140deg, #3A1818 0%, #5A2A2A 55%, #3A1818 100%); color: #E8D0C8; }
  .shelf-cover.book.green { background: linear-gradient(140deg, #14241A 0%, #1E3A28 55%, #14241A 100%); color: #D2E0D4; }
  .shelf-cover.book.plum { background: linear-gradient(140deg, #241422 0%, #3A1E35 55%, #241422 100%); color: #E0D0DA; }

  .shelf-cover.book::before {
    content: '';
    position: absolute;
    top: 10px;
    bottom: 10px;
    left: 4px;
    width: 1.5px;
    background: rgba(255, 255, 255, 0.15);
  }

  .shelf-cover.book .sc-title {
    font-size: 16px;
    line-height: 1.08;
    font-weight: 500;
    margin-bottom: 8px;
  }

  .shelf-cover.book .sc-author {
    font-style: italic;
    font-size: 10px;
    opacity: 0.85;
    border-top: 1px solid rgba(255, 255, 255, 0.15);
    padding-top: 5px;
  }

  /* ── Podcast ───────────────────────────────── */
  .shelf-cover.podcast {
    background: linear-gradient(135deg, #2A1E44 0%, #4E3A6E 100%);
    color: #D8D0E6;
  }
  .shelf-cover.podcast.orange {
    background: linear-gradient(135deg, #4E2812 0%, #8B4A20 100%);
    color: #F5D9B8;
  }

  .shelf-cover.podcast::after {
    content: '';
    position: absolute;
    top: 38%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 55%;
    aspect-ratio: 1/1;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.18);
    background: radial-gradient(circle, rgba(255, 255, 255, 0.1) 0%, transparent 70%);
  }

  .shelf-cover.podcast .sc-title {
    font-family: var(--font-serif);
    font-weight: 500;
    font-size: 13px;
    line-height: 1.12;
  }

  .shelf-cover.podcast .sc-author {
    font-family: var(--font-sans);
    font-size: 10px;
    opacity: 0.75;
    margin-top: 4px;
    font-style: italic;
  }

  /* ── Essay ─────────────────────────────────── */
  .shelf-cover.essay {
    background: var(--surface-warm);
    color: var(--ink);
  }

  .shelf-cover.essay .type-chip {
    background: rgba(21, 19, 15, 0.08);
    border-color: rgba(21, 19, 15, 0.15);
    color: var(--ink-soft);
  }

  .essay-mark {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 44px;
    color: var(--brand-accent);
    line-height: 1;
    margin-bottom: 8px;
  }

  .shelf-cover.essay .sc-title {
    font-family: var(--font-serif);
    font-weight: 500;
    font-size: 13px;
    line-height: 1.18;
  }

  .shelf-cover.essay .sc-author {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 10px;
    color: var(--ink-fade);
    margin-top: 3px;
  }

  /* ── Paper ─────────────────────────────────── */
  .shelf-cover.paper {
    background: #EFEEE8;
    color: var(--ink-soft);
    padding-bottom: 18px;
  }

  .shelf-cover.paper .type-chip {
    background: rgba(21, 19, 15, 0.06);
    border-color: rgba(21, 19, 15, 0.12);
    color: var(--ink-soft);
  }

  .paper-lines {
    position: absolute;
    top: 42px;
    left: 14px;
    right: 14px;
    bottom: 48px;
    background: repeating-linear-gradient(
      transparent,
      transparent 6px,
      rgba(21, 19, 15, 0.1) 6px,
      rgba(21, 19, 15, 0.1) 7px
    );
    opacity: 0.55;
  }

  .shelf-cover.paper .sc-title {
    font-family: var(--font-mono);
    font-weight: 500;
    font-size: 11px;
    line-height: 1.3;
  }

  .shelf-cover.paper .sc-author {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 10px;
    color: var(--ink-fade);
    margin-top: 3px;
  }

  /* ── Archive ───────────────────────────────── */
  .shelf-cover.archive {
    background: linear-gradient(140deg, #D8C8A4 0%, #BFAF8C 100%);
    color: #3A2E18;
  }

  .shelf-cover.archive .type-chip {
    background: rgba(58, 46, 24, 0.15);
    border-color: rgba(58, 46, 24, 0.3);
    color: #3A2E18;
  }

  .archive-mark {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 36px;
    color: rgba(58, 46, 24, 0.5);
    line-height: 1;
    margin-bottom: 8px;
  }

  .shelf-cover.archive .sc-title {
    font-family: var(--font-serif);
    font-weight: 500;
    font-style: italic;
    font-size: 13px;
    line-height: 1.15;
  }

  .shelf-cover.archive .sc-author {
    font-family: var(--font-mono);
    font-size: 9px;
    color: rgba(58, 46, 24, 0.65);
    margin-top: 3px;
    letter-spacing: 0.04em;
  }

  /* ── Status ────────────────────────────────── */
  .shelf-status {
    position: absolute;
    top: 10px;
    right: 10px;
    padding: 2px 8px;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    background: var(--brand-accent);
    color: var(--surface);
    font-weight: 500;
    border-radius: 2px;
    z-index: 3;
  }

  .shelf-status.open {
    background: #7CAE7A;
  }

  .shelf-status.week {
    background: var(--brand-accent);
  }

  /* ── Meta below cover ─────────────────────── */
  .shelf-meta {
    padding: 12px 14px 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .shelf-tile-title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 13px;
    line-height: 1.25;
    color: var(--ink);
  }

  .shelf-tile-author {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 12px;
    color: var(--ink-fade);
  }

  .shelf-tile-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 14px 10px;
    border-top: 1px dotted rgba(21, 19, 15, 0.08);
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
  }

  .shelf-tile-foot .dots {
    display: flex;
  }

  .shelf-tile-foot .overlap {
    margin-left: -5px;
  }

  .shelf-tile-foot :global(.room-member-avatar) {
    box-shadow: 0 0 0 1px var(--surface);
  }
</style>
