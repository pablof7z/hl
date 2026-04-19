<script lang="ts">
  import FilterRow from './FilterRow.svelte';
  import DiscussionRow from './DiscussionRow.svelte';

  const FILTER_PILLS = ['All', 'Books', 'Podcasts', 'Articles'];

  const seedDiscussions = [
    {
      id: 'd1',
      type: 'books',
      memberColorIndex: 1,
      memberName: 'craig_烈日',
      preview:
        "The chapter on digital governance is the most prescient. We're living through exactly the transition they described.",
      replyCount: 12,
      lastActivity: '2h ago'
    },
    {
      id: 'd2',
      type: 'articles',
      memberColorIndex: 3,
      memberName: 'nickand',
      preview:
        'Did anyone else notice the parallel with Taleb? The sovereign individual concept maps directly onto antifragility.',
      replyCount: 8,
      lastActivity: '5h ago'
    },
    {
      id: 'd3',
      type: 'podcasts',
      memberColorIndex: 5,
      memberName: 'Lyn Alden',
      preview:
        'The economic predictions held up surprisingly well. The geopolitical predictions are still unfolding.',
      replyCount: 6,
      lastActivity: '1d ago'
    }
  ];

  let activePill = $state('All');

  const filteredDiscussions = $derived.by(() => {
    if (activePill === 'All') return seedDiscussions;
    return seedDiscussions.filter((d) => d.type === activePill.toLowerCase());
  });

  function handleSeeAll() {
    console.log('see all discussions — stub for M5+');
  }
</script>

<div class="discussions-tab">
  <FilterRow
    pills={FILTER_PILLS}
    {activePill}
    onToggle={(label) => (activePill = label)}
  />

  <div class="discussions-list" role="list">
    {#each filteredDiscussions as discussion (discussion.id)}
      <div role="listitem">
        <DiscussionRow
          id={discussion.id}
          memberColorIndex={discussion.memberColorIndex}
          memberName={discussion.memberName}
          preview={discussion.preview}
          replyCount={discussion.replyCount}
          lastActivity={discussion.lastActivity}
        />
      </div>
    {/each}
  </div>

  <div class="discussions-footer">
    <button class="see-all-link" type="button" onclick={handleSeeAll}>
      See all discussions →
    </button>
  </div>
</div>

<style>
  .discussions-tab {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .discussions-list {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .discussions-footer {
    padding-top: 4px;
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
    text-decoration: none;
  }

  .see-all-link:hover {
    text-decoration: underline;
  }

  .see-all-link:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
