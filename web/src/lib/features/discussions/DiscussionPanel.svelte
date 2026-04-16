<script lang="ts">
  import { browser } from '$app/environment';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    buildCommentTree,
    commentFromEvent,
    discussionFilterForContext,
    type CommentRecord,
    type CommentThread,
    type DiscussionRootContext
  } from './discussion';
  import CommentComposer from './CommentComposer.svelte';
  import CommentThreadRenderer from './CommentThread.svelte';

  let {
    groupId,
    rootContext,
    compact = false,
    maxVisible = 0,
    showHeader = true
  }: {
    groupId: string;
    rootContext: DiscussionRootContext;
    compact?: boolean;
    maxVisible?: number;
    showHeader?: boolean;
  } = $props();

  let optimisticComments = $state<CommentRecord[]>([]);
  let replyingTo = $state<CommentRecord | undefined>(undefined);
  let showAll = $state(false);

  const filter = $derived(discussionFilterForContext(groupId, rootContext));

  const commentFeed = ndk.$subscribe(() => {
    if (!browser) return undefined;

    return {
      filters: [filter],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const allComments = $derived.by(() => {
    const fromRelay = [...commentFeed.events]
      .map((event) => commentFromEvent(event))
      .filter((comment) => comment.content);

    const relayIds = new Set(fromRelay.map((c) => c.eventId));
    const newOptimistic = optimisticComments.filter((c) => !relayIds.has(c.eventId));

    return [...fromRelay, ...newOptimistic];
  });

  const threads = $derived<CommentThread[]>(buildCommentTree(allComments));

  const visibleThreads = $derived.by(() => {
    if (maxVisible <= 0 || showAll) return threads;
    return threads.slice(0, maxVisible);
  });

  const hiddenCount = $derived(
    maxVisible > 0 && !showAll ? Math.max(0, threads.length - maxVisible) : 0
  );

  function handleReply(comment: CommentRecord) {
    replyingTo = comment;
  }

  function handleCancelReply() {
    replyingTo = undefined;
  }

  function handlePublished(comment: CommentRecord) {
    optimisticComments = [...optimisticComments, comment];
    replyingTo = undefined;
  }
</script>

<div class="discussion-panel" class:compact>
  {#if showHeader}
    <div class="discussion-header">
      <h3>Discussion</h3>
      <span class="comment-count">{allComments.length} comment{allComments.length === 1 ? '' : 's'}</span>
    </div>
  {/if}

  <CommentComposer
    {groupId}
    {rootContext}
    {replyingTo}
    onPublished={handlePublished}
    onCancelReply={handleCancelReply}
  />

  {#if visibleThreads.length > 0}
    <CommentThreadRenderer threads={visibleThreads} onReply={handleReply} />
  {:else if commentFeed.eosed}
    <div class="discussion-empty">
      <p>No comments yet. Start the conversation.</p>
    </div>
  {:else}
    <div class="discussion-empty">
      <p>Loading comments…</p>
    </div>
  {/if}

  {#if hiddenCount > 0}
    <button type="button" class="view-all" onclick={() => (showAll = true)}>
      View all ({threads.length} threads)
    </button>
  {/if}
</div>

<style>
  .discussion-panel {
    display: grid;
    gap: 1rem;
  }

  .discussion-panel.compact {
    gap: 0.7rem;
    font-size: 0.92em;
  }

  .discussion-header {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.35rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .compact h3 {
    font-size: 1.05rem;
  }

  .comment-count {
    display: inline-flex;
    align-items: center;
    min-height: 1.8rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
  }

  .discussion-empty {
    padding: 0.85rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface-soft);
  }

  .discussion-empty p {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .view-all {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.85rem;
    border: 0;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    width: fit-content;
  }

  .view-all:hover {
    background: color-mix(in srgb, var(--accent) 10%, white);
  }
</style>
