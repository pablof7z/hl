<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import BookCoverLg from './BookCoverLg.svelte';
  import { memberTint } from '../utils/colors';

  interface Reader {
    pubkey: string;
    colorIndex: number;
  }

  interface Stat {
    value: string;
    label: string;
  }

  let {
    title,
    subtitle,
    coverTitle,
    coverAuthor,
    coverKicker,
    coverVariant = 'dark',
    stats,
    readers,
    readersNote,
    openHref = '#',
    continueHref = '#'
  }: {
    title: string;
    subtitle?: string;
    coverTitle: string;
    coverAuthor?: string;
    coverKicker?: string;
    coverVariant?: 'dark' | 'red' | 'blue' | 'green' | 'plum';
    stats?: Stat[];
    readers?: Reader[];
    readersNote?: string;
    openHref?: string;
    continueHref?: string;
  } = $props();
</script>

<div class="pinned-top">
  <div class="cover-slot">
    <BookCoverLg
      title={coverTitle}
      author={coverAuthor}
      kicker={coverKicker}
      variant={coverVariant}
    />
  </div>

  <div class="pin-meta">
    <h3>{title}</h3>
    {#if subtitle}<div class="subt">{subtitle}</div>{/if}

    {#if stats && stats.length}
      <div class="pin-stats">
        {#each stats as stat (stat.label)}
          <span><b>{stat.value}</b> {stat.label}</span>
        {/each}
      </div>
    {/if}

    {#if readers && readers.length}
      <div class="pin-readers">
        {#each readers as reader (reader.pubkey)}
          <User.Root {ndk} pubkey={reader.pubkey}>
            <span
              class="room-member-avatar"
              style:--mav-size="22px"
              style:--mav-ring={memberTint(reader.colorIndex)}
              style:--mav-ring-width="1.5px"
            >
              <User.Avatar />
            </span>
          </User.Root>
        {/each}
        {#if readersNote}<span class="note">{readersNote}</span>{/if}
      </div>
    {/if}
  </div>

  <div class="pin-actions">
    <a href={openHref} class="pin-action">Open artifact</a>
    <a href={continueHref} class="pin-action filled">Continue reading</a>
  </div>
</div>

<style>
  .pinned-top {
    display: grid;
    grid-template-columns: 140px 1fr auto;
    gap: 28px;
    padding: 28px 32px 24px;
    align-items: start;
    border-bottom: 1px solid var(--rule);
  }

  @media (max-width: 760px) {
    .pinned-top {
      grid-template-columns: 100px 1fr;
      gap: 20px;
      padding: 20px;
    }
    .pin-actions {
      grid-column: 1 / -1;
      justify-self: start;
    }
  }

  .cover-slot {
    width: 140px;
  }

  @media (max-width: 760px) {
    .cover-slot {
      width: 100px;
    }
  }

  .pin-meta h3 {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 26px;
    line-height: 1.15;
    margin: 0 0 4px;
    color: var(--ink);
    letter-spacing: -0.01em;
  }

  .subt {
    font-family: var(--font-sans);
    font-style: italic;
    font-weight: 400;
    font-size: 14px;
    color: var(--ink-fade);
    margin-bottom: 18px;
  }

  .pin-stats {
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ink-fade);
  }

  .pin-stats b {
    color: var(--ink);
    font-weight: 600;
    font-size: 14px;
  }

  .pin-readers {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    font-size: 12px;
    color: var(--ink-fade);
  }

  .pin-readers :global(.room-member-avatar) {
    box-shadow: 0 0 0 1px var(--surface);
  }

  .note {
    font-family: var(--font-sans);
    font-size: 12.5px;
    color: var(--ink-fade);
    margin-left: 8px;
    font-style: italic;
  }

  .pin-actions {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .pin-action {
    padding: 8px 14px;
    border: 1px solid var(--rule);
    background: var(--surface);
    color: var(--ink-soft);
    font-size: 12px;
    letter-spacing: 0.01em;
    font-family: var(--font-sans);
    text-decoration: none;
    font-weight: 500;
    transition: all var(--transition);
    white-space: nowrap;
  }

  .pin-action:hover {
    border-color: var(--brand-accent);
    color: var(--brand-accent);
  }

  .pin-action.filled {
    background: var(--ink);
    color: var(--surface);
    border-color: var(--ink);
  }

  .pin-action.filled:hover {
    background: var(--brand-accent);
    border-color: var(--brand-accent);
    color: var(--surface);
  }
</style>
