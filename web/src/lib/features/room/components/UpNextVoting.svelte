<script lang="ts">
  interface VoteItem {
    id: string;
    title: string;
    source?: string;
    voteCount: number;
  }

  let {
    items,
    closesText = 'Voting closes Sunday, 9pm.',
    castHref = '#',
    seeAllHref = '#'
  }: {
    items: VoteItem[];
    closesText?: string;
    castHref?: string;
    seeAllHref?: string;
  } = $props();

  const MAX_DOTS = 5;
</script>

<div class="sb-card">
  <div class="sb-head">
    <span>Up next · voting</span>
    <a href={seeAllHref} class="sb-link">see all →</a>
  </div>

  {#each items as item, i (item.id)}
    <div class="vote-row">
      <div class="vote-pos">{String(i + 1).padStart(2, '0')}</div>
      <div class="vote-body">
        <div class="vt-title">{item.title}</div>
        {#if item.source}<div class="vt-source">{item.source}</div>{/if}
      </div>
      <div class="vote-tally">
        {#each Array(Math.max(MAX_DOTS, item.voteCount)) as _, i (i)}
          <span class="dot" class:empty={i >= item.voteCount} class:hide={i >= Math.max(1, item.voteCount) && i >= MAX_DOTS - 1}></span>
        {/each}
        <span class="num">{item.voteCount}</span>
      </div>
    </div>
  {/each}

  <div class="vote-close">
    <span>{closesText}</span>
    <a href={castHref}>cast yours →</a>
  </div>
</div>

<style>
  .sb-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 20px 22px;
  }

  .sb-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-fade);
    padding-bottom: 12px;
    border-bottom: 1px dotted var(--rule);
    margin-bottom: 14px;
  }

  .sb-link {
    color: var(--brand-accent);
    text-transform: none;
    letter-spacing: 0.02em;
    font-size: 11px;
    text-decoration: none;
    font-family: var(--font-sans);
    font-weight: 500;
  }

  .sb-link:hover {
    text-decoration: underline;
  }

  .vote-row {
    display: grid;
    grid-template-columns: 24px 1fr auto;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
    align-items: center;
  }

  .vote-row:last-of-type {
    border-bottom: none;
  }

  .vote-pos {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    text-align: center;
  }

  .vt-title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 14px;
    line-height: 1.2;
    color: var(--ink);
  }

  .vt-source {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 12px;
    color: var(--ink-fade);
    margin-top: 1px;
  }

  .vote-tally {
    display: flex;
    gap: 3px;
    align-items: center;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--marker);
  }

  .dot.empty {
    background: var(--rule);
  }

  .dot.hide {
    display: none;
  }

  .num {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    margin-left: 4px;
    font-weight: 500;
  }

  .vote-close {
    padding-top: 12px;
    margin-top: 8px;
    border-top: 1px dashed var(--rule);
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .vote-close a {
    color: var(--brand-accent);
    font-size: 12px;
    font-weight: 500;
    text-decoration: none;
  }

  .vote-close a:hover {
    text-decoration: underline;
  }
</style>
