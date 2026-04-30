<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKEvent, NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import {
    buildCommentTree,
    commentFromEvent,
    type CommentRecord,
    type CommentThread
  } from '$lib/features/discussions/discussion';
  import CommentThreadRenderer from '$lib/features/discussions/CommentThread.svelte';

  /// Public NIP-22 thread on a highlight (kind:9802). Filters by `#E:<id>`
  /// only — no group scope, so the same set of comments is visible to
  /// anyone landing on the public share URL. Composer requires a signer
  /// and publishes a kind:1111 reply rooted at the highlight.

  let {
    highlightEventId,
    seedComments = []
  }: {
    highlightEventId: string;
    seedComments?: NDKEvent[];
  } = $props();

  let optimisticComments = $state<CommentRecord[]>([]);
  let replyingTo = $state<CommentRecord | undefined>(undefined);
  let draft = $state('');
  let publishing = $state(false);
  let errorMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);

  const commentFeed = ndk.$subscribe(() => {
    if (!browser || !highlightEventId) return undefined;
    return {
      filters: [{ kinds: [1111], '#E': [highlightEventId], limit: 200 }],
      closeOnEose: false
    };
  });

  const seeded = $derived(seedComments.map((event) => commentFromEvent(event)));

  const allComments = $derived.by(() => {
    const fromRelay = [...commentFeed.events]
      .map((event) => commentFromEvent(event))
      .filter((comment) => comment.content);

    const seenIds = new Set(fromRelay.map((c) => c.eventId));
    const fromSeed = seeded.filter((c) => !seenIds.has(c.eventId));
    for (const c of fromSeed) seenIds.add(c.eventId);
    const newOptimistic = optimisticComments.filter((c) => !seenIds.has(c.eventId));

    return [...fromSeed, ...fromRelay, ...newOptimistic];
  });

  const threads = $derived<CommentThread[]>(buildCommentTree(allComments));

  function handleReply(comment: CommentRecord) {
    replyingTo = comment;
  }

  function cancelReply() {
    replyingTo = undefined;
  }

  async function submit() {
    if (!currentUser) {
      errorMessage = 'Sign in to post a comment.';
      return;
    }
    if (!ndk.signer) {
      errorMessage = 'Sign in to post a comment.';
      return;
    }
    const content = draft.trim();
    if (!content) return;

    publishing = true;
    errorMessage = '';
    try {
      const comment = await publishHighlightComment({
        highlightEventId,
        parentComment: replyingTo,
        content
      });
      optimisticComments = [...optimisticComments, comment];
      draft = '';
      replyingTo = undefined;
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : 'Failed to publish.';
    } finally {
      publishing = false;
    }
  }

  async function publishHighlightComment(input: {
    highlightEventId: string;
    parentComment?: CommentRecord;
    content: string;
  }): Promise<CommentRecord> {
    if (!ndk.signer) throw new Error('Connect a signer before posting.');

    let commentEvent: NDKEvent;

    if (input.parentComment) {
      const parentEvent = new NDKEvent(ndk, input.parentComment.rawEvent);
      commentEvent = parentEvent.reply(true);
    } else {
      const root = new NDKEvent(ndk);
      root.kind = NDKKind.Highlight;
      root.id = input.highlightEventId;
      commentEvent = root.reply(true);
    }

    commentEvent.content = input.content;

    await commentEvent.sign();
    await commentEvent.publish();

    return commentFromEvent(commentEvent);
  }
</script>

<div class="comments">
  {#if currentUser}
    <div class="composer">
      {#if replyingTo}
        <div class="replying-to">
          Replying to <strong>{replyingTo.pubkey.slice(0, 10)}…</strong>
          <button type="button" class="cancel-reply" onclick={cancelReply}>cancel</button>
        </div>
      {/if}
      <textarea
        class="composer-input"
        placeholder={replyingTo ? 'Write a reply…' : 'Add to the discussion…'}
        bind:value={draft}
        disabled={publishing}
        rows="3"
      ></textarea>
      <div class="composer-actions">
        {#if errorMessage}
          <span class="error">{errorMessage}</span>
        {/if}
        <button
          type="button"
          class="publish"
          onclick={submit}
          disabled={publishing || !draft.trim()}
        >
          {publishing ? 'Posting…' : replyingTo ? 'Reply' : 'Post'}
        </button>
      </div>
    </div>
  {:else}
    <p class="signed-out">
      <a href="/login">Sign in</a> to join the discussion.
    </p>
  {/if}

  {#if threads.length > 0}
    <CommentThreadRenderer {threads} onReply={handleReply} />
  {:else if commentFeed.eosed}
    <p class="empty">No comments yet. Be the first to respond.</p>
  {:else}
    <p class="empty">Loading comments…</p>
  {/if}
</div>

<style>
  .comments {
    display: grid;
    gap: 1rem;
  }

  .composer {
    display: grid;
    gap: 0.6rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.025);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 12px;
  }

  .replying-to {
    font-size: 0.85rem;
    color: var(--color-muted, #695747);
  }

  .cancel-reply {
    margin-left: 0.5rem;
    border: none;
    background: none;
    color: var(--color-accent, #d05a2d);
    font-size: 0.85rem;
    cursor: pointer;
    text-decoration: underline;
  }

  .composer-input {
    width: 100%;
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 8px;
    padding: 0.75rem;
    font: inherit;
    resize: vertical;
    background: var(--color-paper, #fffaf3);
    color: inherit;
  }

  .composer-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.85rem;
  }

  .error {
    font-size: 0.85rem;
    color: #b03a2e;
  }

  .publish {
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
    border: none;
    border-radius: 999px;
    padding: 0.5rem 1.1rem;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .publish:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .signed-out {
    margin: 0;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.025);
    border: 1px dashed var(--color-border, #e8d8cb);
    border-radius: 12px;
    color: var(--color-muted, #695747);
    font-size: 0.95rem;
  }

  .signed-out a {
    color: var(--color-accent, #d05a2d);
    text-decoration: underline;
    font-weight: 600;
  }

  .empty {
    margin: 0.5rem 0 0 0;
    color: var(--color-muted, #695747);
    font-size: 0.95rem;
  }
</style>
