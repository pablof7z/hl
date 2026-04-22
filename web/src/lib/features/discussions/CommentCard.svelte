<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { relativeTime, type CommentRecord } from './discussion';

  let {
    comment,
    depth = 0,
    onReply
  }: {
    comment: CommentRecord;
    depth?: number;
    onReply?: (comment: CommentRecord) => void;
  } = $props();

  const timeLabel = $derived(comment.createdAt ? relativeTime(comment.createdAt) : '');

  function shortPubkey(value: string): string {
    if (!value) return '';
    return `${value.slice(0, 8)}…${value.slice(-4)}`;
  }
</script>

<div class="card card-border bg-base-100 p-4 gap-2" class:nested={depth > 0}>
  <div class="comment-header">
    <User.Root {ndk} pubkey={comment.pubkey}>
      <a class="comment-author" href={`/profile/${comment.pubkey}`}>
        <User.Avatar class="comment-avatar" />
        <User.Name fallback={shortPubkey(comment.pubkey)} />
      </a>
    </User.Root>

    {#if timeLabel}
      <span class="comment-time">{timeLabel}</span>
    {/if}
  </div>

  <p class="comment-content">{comment.content}</p>

  {#if onReply}
    <div class="comment-actions">
      <button type="button" class="btn btn-ghost btn-sm" onclick={() => onReply?.(comment)}>
        Reply
      </button>
    </div>
  {/if}
</div>

<style>
  .nested {
    border-left: 2px solid var(--accent);
    border-radius: 0 var(--radius-box) var(--radius-box) 0;
  }

  .comment-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .comment-author {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--text-strong);
    font-size: 0.84rem;
    font-weight: 700;
    text-decoration: none;
  }

  .comment-author:hover {
    color: var(--accent);
  }

  :global(.comment-avatar) {
    width: 1.6rem;
    height: 1.6rem;
    border-radius: 50%;
    object-fit: cover;
  }

  .comment-time {
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
  }

  .comment-content {
    margin: 0;
    color: var(--text);
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .comment-actions {
    display: flex;
    gap: 0.4rem;
  }
</style>
