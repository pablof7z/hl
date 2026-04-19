<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    members,
    max = 6
  }: {
    members: Array<{ colorIndex: number }>;
    max?: number;
  } = $props();

  const visible = $derived(members.slice(0, max));
  const overflow = $derived(members.length > max ? members.length - max : 0);
</script>

<div class="member-stack">
  {#each visible as member, i (i)}
    <div class="member-stack-item" style:z-index={i + 1}>
      <MemberDot colorIndex={member.colorIndex} size="md" />
    </div>
  {/each}

  {#if overflow > 0}
    <div class="member-stack-badge">+{overflow}</div>
  {/if}
</div>

<style>
  .member-stack {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .member-stack-item {
    position: relative;
    margin-left: -18px; /* 50% overlap of 36px diameter */
  }

  .member-stack-item:first-child {
    margin-left: 0;
  }

  .member-stack-badge {
    margin-left: 6px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-fade);
    white-space: nowrap;
  }
</style>
