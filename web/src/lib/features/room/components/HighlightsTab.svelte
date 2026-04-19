<script lang="ts">
  import FilterRow from './FilterRow.svelte';
  import HighlightEntry from './HighlightEntry.svelte';
  import HighlightCard from './HighlightCard.svelte';

  const seedHighlightCards = [
    {
      id: 'hc1',
      quote: '"The death of distance" — the communication revolution compresses both time and space.',
      memberColorIndex: 1,
      memberName: 'craig_烈日',
      artifactTitle: 'The Sovereign Individual'
    },
    {
      id: 'hc2',
      quote:
        'The transition from the Industrial Age to the Information Age will be as disruptive as the prior agricultural-to-industrial shift.',
      memberColorIndex: 2,
      memberName: 'dergigi',
      artifactTitle: 'The Sovereign Individual'
    },
    {
      id: 'hc3',
      quote: 'Their framework for understanding historical transitions applies directly to the current era.',
      memberColorIndex: 6,
      memberName: 'nick',
      artifactTitle: 'The Sovereign Individual'
    }
  ];

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

  const allNames = $derived([
    'All',
    ...Array.from(new Set(highlights.map((h) => h.memberName)))
  ]);

  let activePill = $state('All');

  const filtered = $derived(
    activePill === 'All'
      ? highlights
      : highlights.filter((h) => h.memberName === activePill)
  );

  function handleSeeAllHighlights() {
    console.log('see all highlights — stub for M5+');
  }
</script>

<div class="highlights-tab">
  <!-- Highlight card reel -->
  <div class="reel-section">
    <div class="highlight-reel" role="list" aria-label="Highlights reel">
      {#each seedHighlightCards as card (card.id)}
        <HighlightCard
          id={card.id}
          quote={card.quote}
          memberColorIndex={card.memberColorIndex}
          memberName={card.memberName}
          artifactTitle={card.artifactTitle}
        />
      {/each}
    </div>
    <div class="reel-footer">
      <button class="see-all-link" type="button" onclick={handleSeeAllHighlights}>
        See all highlights →
      </button>
    </div>
  </div>

  <!-- Full highlights list with per-member filter -->
  <div class="highlights-list-section">
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
</div>

<style>
  .highlights-tab {
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .reel-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .highlight-reel {
    display: flex;
    flex-direction: row;
    gap: 12px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    padding-bottom: 8px;
    /* Custom scrollbar */
    scrollbar-width: thin;
    scrollbar-color: var(--rule) var(--surface-muted);
  }

  .highlight-reel::-webkit-scrollbar {
    height: 4px;
  }

  .highlight-reel::-webkit-scrollbar-track {
    background: var(--surface-muted);
  }

  .highlight-reel::-webkit-scrollbar-thumb {
    background: var(--rule);
    border-radius: 2px;
  }

  .reel-footer {
    padding-top: 2px;
  }

  .see-all-link {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .see-all-link:hover {
    text-decoration: underline;
  }

  .see-all-link:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: 2px;
  }

  .highlights-list-section {
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
