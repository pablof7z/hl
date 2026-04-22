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

<div
  class="card card-border bg-base-100 gap-2 p-4"
  class:border-l-2={depth > 0}
  class:border-l-primary={depth > 0}
  class:rounded-l-none={depth > 0}
>
  <div class="flex flex-wrap items-center gap-2">
    <User.Root {ndk} pubkey={comment.pubkey}>
      <a
        class="flex items-center gap-1.5 text-sm font-bold text-base-content no-underline hover:text-primary"
        href={`/profile/${comment.pubkey}`}
      >
        <User.Avatar class="!size-6 rounded-full object-cover" />
        <User.Name fallback={shortPubkey(comment.pubkey)} />
      </a>
    </User.Root>

    {#if timeLabel}
      <span class="text-xs font-semibold text-base-content/60">{timeLabel}</span>
    {/if}
  </div>

  <p class="m-0 leading-relaxed whitespace-pre-wrap text-base-content [overflow-wrap:anywhere]">{comment.content}</p>

  {#if onReply}
    <div class="flex gap-1.5">
      <button type="button" class="btn btn-ghost btn-sm" onclick={() => onReply?.(comment)}>
        Reply
      </button>
    </div>
  {/if}
</div>
