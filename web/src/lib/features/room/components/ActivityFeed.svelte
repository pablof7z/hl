<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  interface ActivityItem {
    id: string;
    memberColorIndex: number;
    memberInitials: string;
    memberName: string;
    action: string;
    body: string;
    time: string;
  }

  let {
    items
  }: {
    items: ActivityItem[];
  } = $props();
</script>

<div class="feed">
  {#each items as item (item.id)}
    <div class="feed-row">
      <MemberDot
        colorIndex={item.memberColorIndex}
        initials={item.memberInitials}
        size={24}
        title={item.memberName}
      />
      <div class="f-body">
        <b>{item.memberName}</b>
        <span class="f-action">{item.action}</span>
        {@html item.body}
      </div>
      <div class="f-time">{item.time}</div>
    </div>
  {/each}
</div>

<style>
  .feed {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .feed-row {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    gap: 12px;
    padding: 12px 20px;
    align-items: center;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
    font-size: 13.5px;
  }

  .feed-row:last-child {
    border-bottom: none;
  }

  .f-body {
    color: var(--ink-soft);
    line-height: 1.45;
  }

  .f-body b {
    color: var(--ink);
    font-weight: 600;
  }

  .f-body :global(.f-ref) {
    font-style: italic;
    color: var(--ink);
  }

  .f-action {
    display: inline-block;
    padding: 0 6px;
    font-size: 10px;
    font-family: var(--font-mono);
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--brand-accent);
    margin-right: 6px;
  }

  .f-time {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
  }
</style>
