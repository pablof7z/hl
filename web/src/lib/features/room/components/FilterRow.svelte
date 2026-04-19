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
    display: contents;
  }
</style>
