<script lang="ts">
  import FilterRow from './FilterRow.svelte';
  import HighlightEntry from './HighlightEntry.svelte';

  interface HighlightEntryProps {
    id: string;
    memberColorIndex: number;
    memberName: string;
    quote: string;
    timestamp: string;
  }

  let {
    highlights
  }: {
    highlights: HighlightEntryProps[];
  } = $props();

  const allNames = $derived(
    ['All', ...Array.from(new Set(highlights.map((h) => h.memberName)))]
  );

  let activePill = $state('All');

  const filtered = $derived(
    activePill === 'All'
      ? highlights
      : highlights.filter((h) => h.memberName === activePill)
  );
</script>

<div class="highlights-tab">
  <FilterRow
    pills={allNames}
    {activePill}
    onToggle={(label) => (activePill = label)}
  />

  <div class="highlights-list">
    {#if filtered.length === 0}
      <p class="empty-state">No highlights yet. Be the first to share a passage.</p>
    {:else}
      {#each filtered as highlight (highlight.id)}
        <HighlightEntry
          id={highlight.id}
          memberColorIndex={highlight.memberColorIndex}
          memberName={highlight.memberName}
          quote={highlight.quote}
          timestamp={highlight.timestamp}
        />
      {/each}
    {/if}
  </div>
</div>

<style>
  .highlights-tab {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .highlights-list {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .empty-state {
    font-family: var(--font-sans);
    font-size: 15px;
    color: var(--ink-fade);
    text-align: center;
    padding: 40px 0;
    margin: 0;
  }
</style>
