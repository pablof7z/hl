<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  type ActivityType = 'highlight' | 'note' | 'discussion' | 'joined';

  interface ActivityItem {
    type: ActivityType;
    memberColorIndex: number;
    memberName: string;
    detail: string;
    time: string;
  }

  let {
    items
  }: {
    items: ActivityItem[];
  } = $props();

  const ACTION_LABELS: Record<ActivityType, string> = {
    highlight: 'highlighted',
    note: 'added a note to',
    discussion: 'commented on',
    joined: 'joined the room'
  };
</script>

<div class="activity-feed" role="list">
  {#each items as item, i (i)}
    <div class="activity-row" class:row-alt={i % 2 !== 0} role="listitem">
      <div class="activity-member" aria-hidden="true">
        <MemberDot colorIndex={item.memberColorIndex} size="sm" />
      </div>
      <span class="activity-name">{item.memberName}</span>
      <span class="activity-action">{ACTION_LABELS[item.type]}</span>
      {#if item.type !== 'joined'}
        <span class="activity-detail">{item.detail}</span>
      {/if}
      <span class="activity-time">{item.time}</span>
    </div>
  {/each}
</div>

<style>
  .activity-feed {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .activity-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 9px 12px;
    background-color: var(--surface);
    flex-wrap: wrap;
  }

  .activity-row.row-alt {
    background-color: var(--surface-muted);
  }

  .activity-member {
    flex-shrink: 0;
  }

  .activity-name {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    flex-shrink: 0;
  }

  .activity-action {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    color: var(--ink-soft);
    flex-shrink: 0;
  }

  .activity-detail {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    color: var(--ink-fade);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }

  .activity-time {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    flex-shrink: 0;
    margin-left: auto;
  }
</style>
