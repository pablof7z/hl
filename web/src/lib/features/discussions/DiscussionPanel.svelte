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

<div class="grid gap-4" class:!gap-3={compact} class:text-[0.92em]={compact}>
  {#if showHeader}
    <div class="flex flex-wrap items-baseline gap-2.5">
      <h3
        class="m-0 font-serif leading-tight tracking-tight text-base-content"
        class:text-[1.35rem]={!compact}
        class:text-[1.05rem]={compact}
      >Discussion</h3>
      <span class="inline-flex min-h-7 items-center rounded-full bg-base-200 px-2.5 text-xs font-semibold text-base-content/60">
        {allComments.length} comment{allComments.length === 1 ? '' : 's'}
      </span>
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
    <div class="rounded-2xl border border-base-300 bg-base-200 px-4 py-3.5">
      <p class="m-0 text-sm text-base-content/60">No comments yet. Start the conversation.</p>
    </div>
  {:else}
    <div class="rounded-2xl border border-base-300 bg-base-200 px-4 py-3.5">
      <p class="m-0 text-sm text-base-content/60">Loading comments…</p>
    </div>
  {/if}

  {#if hiddenCount > 0}
    <button
      type="button"
      class="inline-flex min-h-8 w-fit cursor-pointer items-center rounded-full border-0 bg-base-200 px-3.5 text-sm font-semibold text-primary transition-colors hover:bg-primary/10"
      onclick={() => (showAll = true)}
    >
      View all ({threads.length} threads)
    </button>
  {/if}
</div>
