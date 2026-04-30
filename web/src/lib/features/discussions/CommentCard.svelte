<script lang="ts">
  import type { NDKEvent } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { MarkdownEventContent } from '$lib/ndk/ui/markdown-event-content';
  import '$lib/ndk/components/mention';
  import '$lib/ndk/components/embedded-note';
  import '$lib/ndk/components/embedded-article';
  import { relativeTime, type CommentRecord } from './discussion';
  import { setBookmarkEventIdPresence } from '$lib/ndk/lists';
  import {
    publishLike,
    publishUnlike,
    type CommentReactions
  } from './reactions';

  let {
    comment,
    depth = 0,
    onReply,
    reactions,
    bookmarked = false,
    bookmarkListEvent
  }: {
    comment: CommentRecord;
    depth?: number;
    onReply?: (comment: CommentRecord) => void;
    /// Per-comment reaction snapshot computed by the parent thread's
    /// batched subscription. Undefined while the subscription is still
    /// loading or when no reactions exist yet.
    reactions?: CommentReactions;
    bookmarked?: boolean;
    bookmarkListEvent?: NDKEvent | undefined;
  } = $props();

  /// Optimistic local state. We layer these over the upstream
  /// `reactions` snapshot so the UI reflects the user's tap before the
  /// reaction event round-trips through relays.
  let optimisticLike = $state<'liked' | 'unliked' | null>(null);
  let optimisticBookmark = $state<boolean | null>(null);
  let myReactionEventIdLocal = $state<string | undefined>(undefined);
  let busy = $state(false);
  let menuOpen = $state(false);
  let copyStatus = $state('');

  const currentUser = $derived(ndk.$currentUser);

  /// Effective like state — prefer optimistic when set, otherwise the
  /// server snapshot. Same for bookmark.
  const iLiked = $derived(
    optimisticLike != null ? optimisticLike === 'liked' : reactions?.iLiked ?? false
  );
  const likeCount = $derived.by(() => {
    const base = reactions?.likeCount ?? 0;
    if (optimisticLike === 'liked' && !(reactions?.iLiked ?? false)) return base + 1;
    if (optimisticLike === 'unliked' && (reactions?.iLiked ?? false)) return Math.max(0, base - 1);
    return base;
  });
  const isBookmarked = $derived(optimisticBookmark != null ? optimisticBookmark : bookmarked);

  const timeLabel = $derived(comment.createdAt ? relativeTime(comment.createdAt) : '');

  function shortPubkey(value: string): string {
    if (!value) return '';
    return `${value.slice(0, 8)}…${value.slice(-4)}`;
  }

  async function toggleLike() {
    if (!currentUser || busy) return;
    busy = true;
    try {
      if (iLiked) {
        const reactionId = reactions?.myReactionEventId ?? myReactionEventIdLocal;
        optimisticLike = 'unliked';
        if (reactionId) {
          await publishUnlike(ndk, reactionId);
        }
        myReactionEventIdLocal = undefined;
      } else {
        optimisticLike = 'liked';
        const id = await publishLike(ndk, {
          eventId: comment.eventId,
          pubkey: comment.pubkey
        });
        myReactionEventIdLocal = id;
      }
    } catch (error) {
      // Roll back optimistic state.
      optimisticLike = null;
      console.warn('Failed to toggle comment like', error);
    } finally {
      busy = false;
    }
  }

  async function toggleBookmark() {
    if (!currentUser || busy) return;
    busy = true;
    try {
      const next = !isBookmarked;
      optimisticBookmark = next;
      await setBookmarkEventIdPresence(ndk, bookmarkListEvent, comment.eventId, next);
    } catch (error) {
      optimisticBookmark = null;
      console.warn('Failed to toggle comment bookmark', error);
    } finally {
      busy = false;
    }
  }

  async function copyText() {
    try {
      await navigator.clipboard.writeText(comment.content);
      copyStatus = 'Copied';
      setTimeout(() => (copyStatus = ''), 1200);
    } catch {
      copyStatus = 'Copy failed';
      setTimeout(() => (copyStatus = ''), 1200);
    }
  }

  function closeMenuAnd<T extends () => unknown>(fn: T) {
    return () => {
      menuOpen = false;
      return fn();
    };
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

  <MarkdownEventContent
    {ndk}
    content={comment.content}
    class="comment-card-content text-base-content [overflow-wrap:anywhere]"
  />

  <div class="comment-actions">
    <button
      type="button"
      class="action-btn"
      class:active={iLiked}
      onclick={toggleLike}
      disabled={!currentUser || busy}
      aria-label={iLiked ? 'Remove like' : 'Like comment'}
      title={currentUser ? (iLiked ? 'Unlike' : 'Like') : 'Sign in to like'}
    >
      <span class="action-icon" aria-hidden="true">{iLiked ? '♥' : '♡'}</span>
      {#if likeCount > 0}
        <span class="action-count">{likeCount}</span>
      {/if}
    </button>

    <button
      type="button"
      class="action-btn"
      class:active={isBookmarked}
      onclick={toggleBookmark}
      disabled={!currentUser || busy}
      aria-label={isBookmarked ? 'Remove bookmark' : 'Bookmark comment'}
      title={currentUser ? (isBookmarked ? 'Remove bookmark' : 'Bookmark') : 'Sign in to bookmark'}
    >
      <span class="action-icon" aria-hidden="true">{isBookmarked ? '🔖' : '🏷'}</span>
    </button>

    {#if onReply}
      <button type="button" class="action-btn" onclick={() => onReply?.(comment)}>
        Reply
      </button>
    {/if}

    <div class="kebab-wrap">
      <button
        type="button"
        class="action-btn kebab"
        onclick={() => (menuOpen = !menuOpen)}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        aria-label="More actions"
      >
        ⋯
      </button>
      {#if menuOpen}
        <div class="kebab-menu" role="menu">
          <button
            type="button"
            class="kebab-item"
            onclick={closeMenuAnd(toggleLike)}
            disabled={!currentUser || busy}
          >
            {iLiked ? 'Unlike' : 'Like'}
          </button>
          <button
            type="button"
            class="kebab-item"
            onclick={closeMenuAnd(toggleBookmark)}
            disabled={!currentUser || busy}
          >
            {isBookmarked ? 'Unbookmark' : 'Bookmark'}
          </button>
          <button type="button" class="kebab-item" onclick={closeMenuAnd(copyText)}>
            {copyStatus || 'Copy text'}
          </button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  :global(.comment-card-content) {
    line-height: 1.55;
  }
  :global(.comment-card-content p) {
    margin: 0 0 0.5rem;
  }
  :global(.comment-card-content p:last-child) {
    margin-bottom: 0;
  }

  .comment-actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-top: 0.4rem;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.3rem 0.6rem;
    border-radius: 999px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--color-muted, #695747);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }

  .action-btn:hover:not(:disabled) {
    color: var(--color-accent, #d05a2d);
    background: rgba(0, 0, 0, 0.03);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .action-btn.active {
    color: var(--color-accent, #d05a2d);
  }

  .action-icon {
    font-size: 0.95rem;
    line-height: 1;
  }

  .action-count {
    font-variant-numeric: tabular-nums;
    font-size: 0.78rem;
  }

  .kebab-wrap {
    position: relative;
    margin-left: auto;
  }

  .kebab {
    font-size: 1.05rem;
    line-height: 1;
    padding: 0.25rem 0.55rem;
  }

  .kebab-menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    min-width: 9rem;
    padding: 0.3rem;
    background: var(--color-paper, #fffaf3);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 10px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.1);
    z-index: 10;
    display: flex;
    flex-direction: column;
  }

  .kebab-item {
    display: block;
    width: 100%;
    padding: 0.45rem 0.7rem;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .kebab-item:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.04);
  }

  .kebab-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
