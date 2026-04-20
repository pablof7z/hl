<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  type Status = 'active' | 'closed';

  interface Participant {
    colorIndex: number;
    initials: string;
    name?: string;
  }

  let {
    id,
    status = 'active',
    statusLabel,
    title,
    source,
    participants = [],
    replies,
    lastAt,
    href = '#'
  }: {
    id?: string;
    status?: Status;
    statusLabel?: string;
    title: string;
    source?: string;
    participants?: Participant[];
    replies: number;
    lastAt: string;
    href?: string;
  } = $props();
</script>

<a {href} class="disc-row" data-id={id}>
  <div class="dr-dots">
    {#each participants as p, i (i)}
      <span class:overlap={i > 0}>
        <MemberDot
          colorIndex={p.colorIndex}
          initials={p.initials}
          size={26}
          title={p.name}
        />
      </span>
    {/each}
  </div>

  <div class="dr-body">
    <span class="dr-status {status}">{statusLabel ?? (status === 'active' ? 'Active' : 'Closed')}</span>
    <div class="dr-title">{title}</div>
    {#if source}<div class="dr-source">{@html source}</div>{/if}
  </div>

  <div class="dr-stats">
    <div class="dr-replies">
      {#if status === 'active'}<span class="hot">●</span> {/if}
      <b>{replies}</b> {replies === 1 ? 'reply' : 'replies'}
    </div>
    <div class="dr-last">{lastAt}</div>
  </div>
</a>

<style>
  .disc-row {
    display: grid;
    grid-template-columns: 88px 1fr auto;
    gap: 16px;
    padding: 18px 22px;
    border-bottom: 1px solid var(--rule-soft);
    align-items: center;
    text-decoration: none;
    color: inherit;
    transition: background 150ms;
  }

  .disc-row:hover {
    background: var(--bg);
  }

  .disc-row:last-child {
    border-bottom: none;
  }

  .dr-dots {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .dr-dots :global(.member-dot) {
    border: 2px solid var(--surface);
  }

  .overlap {
    margin-left: -8px;
  }

  .dr-body {
    min-width: 0;
  }

  .dr-status {
    display: inline-block;
    padding: 1px 6px;
    border-radius: 2px;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    margin-bottom: 5px;
    font-weight: 500;
  }

  .dr-status.active {
    background: rgba(124, 174, 122, 0.18);
    color: #4A7248;
  }

  .dr-status.closed {
    background: var(--surface-muted);
    color: var(--ink-fade);
  }

  .dr-title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 14.5px;
    color: var(--ink);
    line-height: 1.3;
    margin-bottom: 3px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dr-source {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 12px;
    color: var(--ink-fade);
  }

  .dr-source :global(b) {
    color: var(--ink);
    font-weight: 600;
    font-style: normal;
  }

  .dr-stats {
    text-align: right;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    min-width: 110px;
    flex-shrink: 0;
  }

  .dr-replies {
    font-family: var(--font-sans);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink);
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .hot {
    color: var(--brand-accent);
  }

  .dr-last {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  @media (max-width: 760px) {
    .disc-row {
      grid-template-columns: 1fr auto;
    }
    .dr-dots {
      display: none;
    }
  }
</style>
