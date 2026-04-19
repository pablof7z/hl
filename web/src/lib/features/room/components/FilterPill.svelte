<script lang="ts">
  let {
    label,
    active = false
  }: {
    label: string;
    active?: boolean;
  } = $props();
</script>

<span class="filter-pill" class:filter-pill--active={active}>{label}</span>

<style>
  .filter-pill {
    display: inline-flex;
    align-items: center;
    padding: 4px 12px;
    border-radius: var(--radius-pill);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    background-color: var(--surface-muted);
    color: var(--ink-soft);
    white-space: nowrap;
    user-select: none;
    cursor: pointer;
    border: none;
    transition: none;
  }

  .filter-pill--active {
    background-color: var(--brand-accent);
    color: var(--surface);
  }
</style>
