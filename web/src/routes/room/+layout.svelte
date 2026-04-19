<script lang="ts">
  import type { LayoutData } from './$types';

  const { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();
</script>

{#if data.isRoomEnabled}
  {@render children()}
{:else}
  <div class="coming-soon">
    <p>Room view coming soon</p>
  </div>
{/if}

<style>
  .coming-soon {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 40vh;
    padding: var(--space-8, 2rem);
  }

  .coming-soon p {
    font-family: var(--font-sans);
    font-size: 1rem;
    color: var(--ink-fade);
  }
</style>
