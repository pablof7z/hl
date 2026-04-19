<script lang="ts">
  import type { Snippet } from 'svelte';
  import MemberStack from './MemberStack.svelte';

  let {
    members,
    children
  }: {
    members: Array<{ colorIndex: number; name?: string }>;
    children?: Snippet;
  } = $props();
</script>

<div class="members-sidebar">
  {#if children}
    <div class="sidebar-title">
      {@render children()}
    </div>
  {/if}

  <div class="sidebar-card">
    <div class="sidebar-stack" aria-hidden="true">
      <MemberStack {members} max={members.length} />
    </div>
    <p class="sidebar-count">{members.length} members</p>
  </div>
</div>

<style>
  .members-sidebar {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .sidebar-title {
    font-family: var(--font-serif);
    font-size: clamp(28px, 3vw, 40px);
    font-weight: 400;
    color: var(--ink);
    line-height: 1.1;
  }

  .sidebar-card {
    background: var(--surface);
    border: 4px solid var(--rule);
    border-radius: var(--radius);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }



  .sidebar-count {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    color: var(--ink-fade);
    margin: 0;
  }
</style>
