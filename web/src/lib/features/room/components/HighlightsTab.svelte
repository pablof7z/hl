<script lang="ts">
  import HighlightEntry from './HighlightEntry.svelte';
  import SeeAllLink from './SeeAllLink.svelte';

  interface HighlightRow {
    id: string;
    authorPubkey: string;
    colorIndex: number;
    quote: string;
    location?: string;
    date?: string;
    replies?: number;
  }

  interface MemberFilter {
    pubkey: string;
    colorIndex: number;
    count: number;
  }

  let {
    highlights,
    totalCount,
    memberFilters = [],
    seeAllHref = '#'
  }: {
    highlights: HighlightRow[];
    totalCount?: number;
    memberFilters?: MemberFilter[];
    seeAllHref?: string;
  } = $props();

  let activePubkey = $state<string>('all');

  const filtered = $derived(
    activePubkey === 'all'
      ? highlights
      : highlights.filter((h) => h.authorPubkey === activePubkey)
  );

  const total = $derived(totalCount ?? highlights.length);
</script>

<div class="panel-head">
  <div class="filter-row">
    <button
      type="button"
      class="filter-pill"
      class:on={activePubkey === 'all'}
      onclick={() => (activePubkey = 'all')}
    >
      All <span class="c">{total}</span>
    </button>
    {#each memberFilters as mf (mf.pubkey)}
      <button
        type="button"
        class="filter-pill"
        class:on={activePubkey === mf.pubkey}
        onclick={() => (activePubkey = mf.pubkey)}
        aria-label="Filter by {mf.pubkey.slice(0, 8)}"
      >
        <span class="dot dot-{mf.colorIndex}"></span>
        {mf.pubkey.slice(0, 4)}
        <span class="c">{mf.count}</span>
      </button>
    {/each}
  </div>
  <div class="panel-sort">By position ↓</div>
</div>

<div class="hl-list">
  {#if filtered.length === 0}
    <p class="empty-state">No highlights yet.</p>
  {:else}
    {#each filtered as hl (hl.id)}
      <HighlightEntry
        id={hl.id}
        authorPubkey={hl.authorPubkey}
        colorIndex={hl.colorIndex}
        quote={hl.quote}
        location={hl.location}
        date={hl.date}
        replies={hl.replies}
      />
    {/each}
  {/if}
</div>

<div class="see-all-wrap">
  <SeeAllLink label="See all {total} highlights" href={seeAllHref} />
</div>

<style>
  .panel-head {
    padding: 18px 32px 14px;
    border-bottom: 1px solid var(--rule);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  @media (max-width: 760px) {
    .panel-head { padding: 14px 20px; }
  }

  .filter-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .filter-pill {
    padding: 6px 12px;
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: 999px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-soft);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: all 150ms ease;
  }

  .filter-pill.on {
    background: var(--ink);
    color: var(--surface);
    border-color: var(--ink);
  }

  .filter-pill:hover:not(.on) {
    border-color: var(--ink);
    color: var(--ink);
  }

  .filter-pill .c {
    font-family: var(--font-mono);
    font-size: 10px;
    opacity: 0.7;
    font-weight: 400;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .dot-1 { background: var(--h-amber); }
  .dot-2 { background: var(--h-sage); }
  .dot-3 { background: var(--h-blue); }
  .dot-4 { background: var(--h-rose); }
  .dot-5 { background: var(--h-lilac); }
  .dot-6 { background: var(--h-amber-l); }

  .panel-sort {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
    font-weight: 500;
  }

  .hl-list {
    padding: 0 32px;
  }

  @media (max-width: 760px) { .hl-list { padding: 0 20px; } }

  .empty-state {
    font-family: var(--font-sans);
    font-size: 15px;
    color: var(--ink-fade);
    text-align: center;
    padding: 40px 0;
    margin: 0;
  }

  .see-all-wrap {
    padding: 0 32px 28px;
  }

  @media (max-width: 760px) { .see-all-wrap { padding: 0 20px 24px; } }
</style>
