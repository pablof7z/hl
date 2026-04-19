<script module lang="ts">
  export const TABS = ['Discussions', 'Highlights', 'Notes', 'Members'] as const;
  export type RoomTab = (typeof TABS)[number];
</script>

<script lang="ts">
  let {
    activeTab = 'Discussions',
    onTabChange
  }: {
    activeTab: RoomTab;
    onTabChange: (tab: RoomTab) => void;
  } = $props();

  function handleTabKeydown(event: KeyboardEvent) {
    const currentIndex = TABS.indexOf(activeTab as RoomTab);
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      onTabChange(TABS[(currentIndex + 1) % TABS.length]);
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      onTabChange(TABS[(currentIndex - 1 + TABS.length) % TABS.length]);
    } else if (event.key === 'Home') {
      event.preventDefault();
      onTabChange(TABS[0]);
    } else if (event.key === 'End') {
      event.preventDefault();
      onTabChange(TABS[TABS.length - 1]);
    }
  }
</script>

<div class="tab-strip" role="tablist" aria-label="Room sections">
  {#each TABS as tab (tab)}
    <button
      type="button"
      id={`room-tab-${tab.toLowerCase()}`}
      class="tab-btn"
      class:tab-active={activeTab === tab}
      role="tab"
      aria-selected={activeTab === tab}
      aria-controls={`room-panel-${tab.toLowerCase()}`}
      tabindex={activeTab === tab ? 0 : -1}
      onclick={() => onTabChange(tab)}
      onkeydown={handleTabKeydown}
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
