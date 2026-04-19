<script lang="ts">
  import '$lib/features/room/styles/tokens.css';
  import PinnedArtifact from '$lib/features/room/components/PinnedArtifact.svelte';
  import MembersSidebar from '$lib/features/room/components/MembersSidebar.svelte';
  import TabStrip, { type RoomTab } from '$lib/features/room/components/TabStrip.svelte';
  import DiscussionsTab from '$lib/features/room/components/DiscussionsTab.svelte';

  const seedMembers = [
    { colorIndex: 1, name: 'craig_烈日' },
    { colorIndex: 2, name: 'dergigi' },
    { colorIndex: 3, name: 'nickand' },
    { colorIndex: 4, name: 'Bob Rocket' },
    { colorIndex: 5, name: 'Lyn Alden' },
    { colorIndex: 6, name: 'nick' }
  ];

  const pinnedBook = {
    title: 'The Sovereign Individual',
    author: 'James Dale Davidson & Lord William Rees-Mogg',
    cover:
      'https://images-na.ssl-images-amazon.com/images/S/compressed.photo.goodreads.com/books/1445342743i/26816291.jpg'
  };

  let activeTab = $state<RoomTab>('Discussions');
</script>

<svelte:head>
  <title>Signal vs Noise · Room</title>
</svelte:head>

<div class="room-layout">
  <aside class="room-sidebar">
    <MembersSidebar members={seedMembers}>
      {#snippet children()}
        Signal vs Noise
      {/snippet}
    </MembersSidebar>
  </aside>

  <main class="room-main">
    <PinnedArtifact artifact={pinnedBook} />

    <div class="room-tabs">
      <TabStrip {activeTab} onTabChange={(tab) => (activeTab = tab)} />

      <div class="tab-content">
        <div
          id="room-panel-discussions"
          role="tabpanel"
          aria-labelledby="room-tab-discussions"
          hidden={activeTab !== 'Discussions'}
        >
          <DiscussionsTab />
        </div>
        <div
          id="room-panel-highlights"
          role="tabpanel"
          aria-labelledby="room-tab-highlights"
          hidden={activeTab !== 'Highlights'}
        >
          <p class="tab-stub">Highlights coming soon.</p>
        </div>
        <div
          id="room-panel-notes"
          role="tabpanel"
          aria-labelledby="room-tab-notes"
          hidden={activeTab !== 'Notes'}
        >
          <p class="tab-stub">Notes coming soon.</p>
        </div>
        <div
          id="room-panel-members"
          role="tabpanel"
          aria-labelledby="room-tab-members"
          hidden={activeTab !== 'Members'}
        >
          <p class="tab-stub">Members coming soon.</p>
        </div>
      </div>
    </div>
  </main>
</div>

<style>
  .room-layout {
    display: grid;
    grid-template-columns: var(--grid-sidebar) 1fr;
    gap: var(--grid-gap);
    max-width: var(--container-max);
    padding: 0 var(--container-px);
    margin: 0 auto;
    padding-top: 40px;
    padding-bottom: 80px;
  }

  .room-main {
    display: flex;
    flex-direction: column;
    gap: 32px;
    min-width: 0;
  }

  .room-tabs {
    display: flex;
    flex-direction: column;
  }

  .tab-content {
    padding-top: 8px;
  }

  .tab-stub {
    font-family: var(--font-sans);
    font-size: 14px;
    color: var(--ink-fade);
    padding: 24px 0;
    margin: 0;
  }

  @media (max-width: 768px) {
    .room-layout {
      grid-template-columns: 1fr;
    }

    .room-sidebar {
      order: 2;
    }

    .room-main {
      order: 1;
    }
  }
</style>
