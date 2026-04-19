<script lang="ts">
  const TABS = ['Discussions', 'Highlights', 'Notes', 'Members'] as const;
  type Tab = (typeof TABS)[number];

  let {
    activeTab,
    onTabChange
  }: {
    activeTab: string;
    onTabChange: (tab: string) => void;
  } = $props();
</script>

<div class="tab-strip" role="tablist" aria-label="Room sections">
  {#each TABS as tab (tab)}
    <button
      class="tab-btn"
      class:tab-active={activeTab === tab}
      role="tab"
      aria-selected={activeTab === tab}
      onclick={() => onTabChange(tab)}
    >
      {tab}
    </button>
  {/each}
</div>

<style>
  .tab-strip {
    display: flex;
    border-bottom: 1px solid var(--rule);
    gap: 0;
  }

  .tab-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink-fade);
    background: none;
    border: none;
    border-bottom: 3px solid transparent;
    padding: 12px 18px;
    cursor: pointer;
    margin-bottom: -1px; /* overlap container border */
    transition: none; /* instant — no animation */
  }

  .tab-btn:hover {
    color: var(--ink);
  }

  .tab-btn.tab-active {
    color: var(--ink);
    border-bottom-color: var(--brand-accent);
  }
</style>
