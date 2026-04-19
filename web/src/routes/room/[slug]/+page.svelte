<script lang="ts">
  import '$lib/features/room/styles/tokens.css';
  import PinnedArtifact from '$lib/features/room/components/PinnedArtifact.svelte';
  import MembersSidebar from '$lib/features/room/components/MembersSidebar.svelte';
  import TabStrip, { type RoomTab } from '$lib/features/room/components/TabStrip.svelte';
  import DiscussionsTab from '$lib/features/room/components/DiscussionsTab.svelte';
  import HighlightsTab from '$lib/features/room/components/HighlightsTab.svelte';
  import NotesTab from '$lib/features/room/components/NotesTab.svelte';
  import MembersTable from '$lib/features/room/components/MembersTable.svelte';
  import ArtifactCard from '$lib/features/room/components/ArtifactCard.svelte';
  import ArticleView from '$lib/features/room/components/ArticleView.svelte';
  import PodcastView from '$lib/features/room/components/PodcastView.svelte';
  import UpNextVoting from '$lib/features/room/components/UpNextVoting.svelte';
  import CaptureCta from '$lib/features/room/components/CaptureCta.svelte';

  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  interface ArtifactCardProps {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
    highlightCount?: number;
    discussionCount?: number;
  }

  const seedMembers = [
    { colorIndex: 1, name: 'craig_烈日', joinedAt: 'Mar 2024' },
    { colorIndex: 2, name: 'dergigi', joinedAt: 'Mar 2024' },
    { colorIndex: 3, name: 'nickand', joinedAt: 'Apr 2024' },
    { colorIndex: 4, name: 'Bob Rocket', joinedAt: 'Apr 2024' },
    { colorIndex: 5, name: 'Lyn Alden', joinedAt: 'Jun 2024' },
    { colorIndex: 6, name: 'nick', joinedAt: 'Jun 2024' }
  ];

  const seedHighlights = [
    {
      id: 'h1',
      memberColorIndex: 1,
      memberName: 'craig_烈日',
      quote:
        '"The death of distance" — the communication revolution compresses both time and space, making geography increasingly irrelevant to economic activity.',
      timestamp: '3d ago'
    },
    {
      id: 'h2',
      memberColorIndex: 2,
      memberName: 'dergigi',
      quote:
        'The transition from the Industrial Age to the Information Age will be as disruptive as the transition from the Agricultural to the Industrial Age.',
      timestamp: '5d ago'
    },
    {
      id: 'h3',
      memberColorIndex: 3,
      memberName: 'nickand',
      quote:
        'The sovereign individual will be someone who can earn a living anywhere on earth, unbound by national borders or currency controls.',
      timestamp: '1w ago'
    },
    {
      id: 'h4',
      memberColorIndex: 5,
      memberName: 'Lyn Alden',
      quote:
        'The book correctly predicts many aspects of the 1999–2024 era that we are now living through, including the decline of the nation-state.',
      timestamp: '2w ago'
    },
    {
      id: 'h5',
      memberColorIndex: 6,
      memberName: 'nick',
      quote:
        'Their framework for understanding the transition from agricultural to industrial societies applies directly to the current transition.',
      timestamp: '3w ago'
    }
  ];

  const seedNotes = [
    {
      id: 'n1',
      memberColorIndex: 4,
      memberName: 'Bob Rocket',
      content:
        'Reading this chapter alongside "The Pragmatic Programmer" — the sovereignty framework maps surprisingly well onto software architecture decisions.',
      timestamp: '1d ago'
    },
    {
      id: 'n2',
      memberColorIndex: 1,
      memberName: 'craig_烈日',
      content:
        'This prediction about cyberspace predating the mainstream internet is striking. They wrote this in 1996.',
      timestamp: '4d ago'
    }
  ];

  const pinnedBook = {
    title: 'The Sovereign Individual',
    author: 'James Dale Davidson & Lord William Rees-Mogg',
    cover:
      'https://images-na.ssl-images-amazon.com/images/S/compressed.photo.goodreads.com/books/1445342743i/26816291.jpg'
  };

  const seedArtifacts: ArtifactCardProps[] = [
    {
      id: 'a1',
      type: 'article',
      title: 'The Death of Distance: How the Communications Revolution Is Changing Our Lives',
      author: 'Frances Cairncross',
      highlightCount: 14,
      discussionCount: 5
    },
    {
      id: 'a2',
      type: 'podcast',
      title: 'The Sovereign Individual with James Dale Davidson',
      author: 'What Bitcoin Did',
      highlightCount: 22,
      discussionCount: 8
    },
    {
      id: 'a3',
      type: 'article',
      title: 'Why Nation-States Are Losing Their Grip on the Digital Economy',
      author: 'Balaji Srinivasan',
      highlightCount: 9,
      discussionCount: 3
    }
  ];

  const roomTitle = 'Signal vs Noise';

  const seedUpNext = [
    {
      id: 'u1',
      title: 'The Bitcoin Standard',
      type: 'book' as const,
      voterCount: 4,
      voterColors: [1, 2, 3, 5],
    },
    {
      id: 'u2',
      title: 'The Great Mental Models',
      type: 'podcast' as const,
      voterCount: 3,
      voterColors: [1, 4, 6],
    },
    {
      id: 'u3',
      title: 'Principles for Dealing with the Changing World',
      type: 'article' as const,
      voterCount: 2,
      voterColors: [2, 3],
    },
  ];

  let activeTab = $state<RoomTab>('Discussions');
  let activeView = $state<'room' | 'article' | 'podcast'>('room');
  let selectedArtifact = $state<ArtifactCardProps | null>(null);

  function handleArtifactClick(artifact: ArtifactCardProps) {
    selectedArtifact = artifact;
    activeView = artifact.type === 'podcast' ? 'podcast' : 'article';
  }

  function handleBack() {
    activeView = 'room';
    selectedArtifact = null;
  }
</script>

<svelte:head>
  <title>Signal vs Noise · Room</title>
</svelte:head>

{#if activeView === 'article' && selectedArtifact}
  <div class="view-container">
    <ArticleView
      artifact={selectedArtifact}
      members={seedMembers}
      onBack={handleBack}
    />
  </div>
{:else if activeView === 'podcast' && selectedArtifact}
  <div class="view-container">
    <PodcastView
      artifact={selectedArtifact}
      members={seedMembers}
      onBack={handleBack}
    />
  </div>
{:else}
  <div class="room-layout">
    <aside class="room-sidebar">
      <MembersSidebar members={seedMembers}>
        {#snippet children()}
          {roomTitle}
        {/snippet}
      </MembersSidebar>
      <UpNextVoting items={seedUpNext} />
      <CaptureCta {roomTitle} />
    </aside>

    <main class="room-main">
      <PinnedArtifact artifact={pinnedBook} />

      <!-- Artifacts shelf -->
      <section class="artifacts-shelf" aria-label="Room artifacts">
        <h2 class="shelf-heading">In This Room</h2>
        <div class="artifacts-list">
          {#each seedArtifacts as artifact (artifact.id)}
            <ArtifactCard
              id={artifact.id}
              type={artifact.type}
              title={artifact.title}
              author={artifact.author}
              cover={artifact.cover}
              highlightCount={artifact.highlightCount}
              discussionCount={artifact.discussionCount}
              onArtifactClick={handleArtifactClick}
            />
          {/each}
        </div>
      </section>

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
            <HighlightsTab highlights={seedHighlights} onHighlightClick={handleArtifactClick} />
          </div>
          <div
            id="room-panel-notes"
            role="tabpanel"
            aria-labelledby="room-tab-notes"
            hidden={activeTab !== 'Notes'}
          >
            <NotesTab notes={seedNotes} />
          </div>
          <div
            id="room-panel-members"
            role="tabpanel"
            aria-labelledby="room-tab-members"
            hidden={activeTab !== 'Members'}
          >
            <MembersTable members={seedMembers} />
          </div>
        </div>
      </div>
    </main>
  </div>
{/if}

<style>
  .view-container {
    max-width: var(--container-max);
    padding: 0 var(--container-px);
    margin: 0 auto;
  }

  .room-layout {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--grid-gap);
    max-width: var(--container-max);
    padding: 0 var(--container-px);
    margin: 0 auto;
    padding-top: 24px;
    padding-bottom: 80px;
  }

  .room-sidebar {
    order: 2;
    position: static;
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-height: none;
    overflow-y: visible;
  }

  .room-main {
    order: 1;
    display: flex;
    flex-direction: column;
    gap: 32px;
    min-width: 0;
  }

  /* Artifacts shelf */
  .artifacts-shelf {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .shelf-heading {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0;
  }

  .artifacts-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .room-tabs {
    display: flex;
    flex-direction: column;
  }

  .tab-content {
    padding-top: 8px;
  }

  @media (min-width: 768px) {
    .room-layout {
      grid-template-columns: var(--grid-sidebar) 1fr;
      padding-top: 40px;
    }

    .room-sidebar {
      order: unset;
      position: sticky;
      top: 24px;
      max-height: calc(100vh - 48px);
      overflow-y: auto;
    }

    .room-main {
      order: unset;
    }
  }
</style>
