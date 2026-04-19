<script lang="ts">
  import FilterPill from './FilterPill.svelte';

  let {
    pills,
    activePill,
    onToggle
  }: {
    pills: string[];
    activePill?: string;
    onToggle: (label: string) => void;
  } = $props();
</script>

<div class="filter-row">
  {#each pills as pill (pill)}
    <button
      class="filter-row-btn"
      onclick={() => onToggle(pill)}
      type="button"
      aria-pressed={pill === activePill}
    >
      <FilterPill label={pill} active={pill === activePill} />
    </button>
  {/each}
</div>

<style>
  .filter-row {
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }

  .filter-row-btn {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    cursor: pointer;
    border-radius: var(--radius-pill);
  }

  .filter-row-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
  }
</style>
