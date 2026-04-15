<script lang="ts">
  import CommentCard from './CommentCard.svelte';
  import CommentThread from './CommentThread.svelte';
  import type { CommentRecord, CommentThread as CommentThreadType } from './discussion';

  let {
    threads,
    depth = 0,
    onReply
  }: {
    threads: CommentThreadType[];
    depth?: number;
    onReply?: (comment: CommentRecord) => void;
  } = $props();

  let expandedDeep = $state<Set<string>>(new Set());

  function toggleExpand(eventId: string) {
    const next = new Set(expandedDeep);
    if (next.has(eventId)) {
      next.delete(eventId);
    } else {
      next.add(eventId);
    }
    expandedDeep = next;
  }

  function countReplies(thread: CommentThreadType): number {
    let count = thread.replies.length;
    for (const reply of thread.replies) {
      count += countReplies(reply);
    }
    return count;
  }
</script>

<div class="thread-list" class:indented={depth > 0}>
  {#each threads as thread (thread.comment.eventId)}
    <div class="thread-node">
      <CommentCard comment={thread.comment} {depth} {onReply} />

      {#if thread.replies.length > 0}
        {#if depth < 1}
          <CommentThread threads={thread.replies} depth={depth + 1} {onReply} />
        {:else if expandedDeep.has(thread.comment.eventId)}
          <CommentThread threads={thread.replies} depth={depth + 1} {onReply} />
        {:else}
          <button
            type="button"
            class="expand-button"
            onclick={() => toggleExpand(thread.comment.eventId)}
          >
            View {countReplies(thread)} more
            {countReplies(thread) === 1 ? 'reply' : 'replies'}
          </button>
        {/if}
      {/if}
    </div>
  {/each}
</div>

<style>
  .thread-list {
    display: grid;
    gap: 0.6rem;
  }

  .thread-list.indented {
    margin-left: 1.5rem;
  }

  .thread-node {
    display: grid;
    gap: 0.6rem;
  }

  .expand-button {
    display: inline-flex;
    align-items: center;
    margin-left: 1.5rem;
    min-height: 1.8rem;
    padding: 0 0.65rem;
    border: 0;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--accent);
    font-size: 0.76rem;
    font-weight: 600;
    cursor: pointer;
    width: fit-content;
  }

  .expand-button:hover {
    background: color-mix(in srgb, var(--accent) 12%, white);
  }
</style>
