<script lang="ts">
  import MemberStack from './MemberStack.svelte';

  interface UpNextItem {
    id: string;
    title: string;
    type: 'book' | 'podcast' | 'article';
    voterCount: number;
    voterColors: number[];
  }

  let { items }: { items: UpNextItem[] } = $props();

  // Track which items the current user has voted on
  let voted = $state<Set<string>>(new Set());

  function toggleVote(item: UpNextItem) {
    const next = new Set(voted);
    if (next.has(item.id)) {
      next.delete(item.id);
      console.log(`Unvoted for "${item.title}" — stub, real voting in post-M9`);
    } else {
      next.add(item.id);
      console.log(`Voted for "${item.title}" — stub, real voting in post-M9`);
    }
    voted = next;
  }

  function displayCount(item: UpNextItem): number {
    const delta = voted.has(item.id) ? 1 : 0;
    return item.voterCount + delta;
  }
</script>

<div class="upnext-card">
  <p class="kicker">UP NEXT</p>

  <ul class="upnext-list" role="list">
    {#each items as item (item.id)}
      {@const isVoted = voted.has(item.id)}
      <li class="upnext-row">
        <button
          type="button"
          class="vote-btn"
          class:voted={isVoted}
          onclick={() => toggleVote(item)}
          aria-pressed={isVoted}
          aria-label="Vote for {item.title}"
        >
          <span class="vote-count">{displayCount(item)}</span>
        </button>

        <div class="item-info">
          <span class="item-title">{item.title}</span>
          <div class="item-meta">
            <span class="item-type">{item.type.toUpperCase()}</span>
            <MemberStack
              members={item.voterColors.map((c) => ({ colorIndex: c }))}
              max={4}
            />
          </div>
        </div>
      </li>
    {/each}
  </ul>
</div>

<style>
  .upnext-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius, 4px);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 400;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0;
  }

  .upnext-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .upnext-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 6px;
    border-radius: var(--radius, 4px);
    transition: none;
  }

  .upnext-row:hover {
    background: var(--surface-muted);
  }

  .vote-btn {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--surface-muted);
    border: 1px solid var(--rule);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 0;
    transition: none;
  }

  .vote-btn.voted {
    background: var(--brand-accent);
    border-color: var(--brand-accent);
  }

  .vote-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
  }

  .vote-btn.voted .vote-count {
    color: var(--surface);
  }

  .vote-count {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 400;
    color: var(--ink-fade);
    line-height: 1;
  }

  .item-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .item-title {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    line-height: 1.3;
  }

  .item-meta {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .item-type {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 400;
    color: var(--ink-fade);
    letter-spacing: 0.05em;
  }
</style>
