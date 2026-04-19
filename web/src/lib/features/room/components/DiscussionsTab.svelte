<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  const mockDiscussions = [
    {
      id: 1,
      colorIndex: 1,
      name: 'craig_烈日',
      time: '2h ago',
      text: 'The thesis on sovereign individuals resonates deeply with the Nostr ethos — censorship-resistance as first principle.'
    },
    {
      id: 2,
      colorIndex: 5,
      name: 'Lyn Alden',
      time: '5h ago',
      text: 'Davidson & Rees-Mogg got a lot right, but underestimated the network effects of existing institutions. Still, a prescient read.'
    },
    {
      id: 3,
      colorIndex: 2,
      name: 'dergigi',
      time: 'yesterday',
      text: 'Chapter 4 is the one that aged best. The idea that violence becomes economically irrational for extractive states when individuals are mobile and cryptographic.'
    }
  ];
</script>

<div class="discussions-tab">
  {#each mockDiscussions as discussion (discussion.id)}
    <div class="discussion-entry">
      <div class="discussion-avatar" aria-hidden="true">
        <MemberDot colorIndex={discussion.colorIndex} size="md" />
      </div>
      <div class="discussion-content">
        <div class="discussion-header">
          <span class="discussion-name">{discussion.name}</span>
          <span class="discussion-time">{discussion.time}</span>
        </div>
        <p class="discussion-text">{discussion.text}</p>
      </div>
    </div>
  {/each}
</div>

<style>
  .discussions-tab {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .discussion-entry {
    display: flex;
    gap: 14px;
    padding: 18px 0;
    border-bottom: 1px solid var(--rule-soft);
  }

  .discussion-entry:last-child {
    border-bottom: none;
  }

  .discussion-avatar {
    flex-shrink: 0;
    padding-top: 2px;
  }

  .discussion-content {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }

  .discussion-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .discussion-name {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
  }

  .discussion-time {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 400;
    color: var(--ink-fade);
  }

  .discussion-text {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 400;
    color: var(--ink-soft);
    line-height: 1.55;
    margin: 0;
  }
</style>
