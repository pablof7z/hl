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

<div class="tabs tabs-border overflow-x-auto" role="tablist" aria-label="Room sections">
  {#each TABS as tab (tab)}
    <button
      type="button"
      id={`room-tab-${tab.toLowerCase()}`}
      class="tab"
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
