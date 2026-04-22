<script lang="ts">
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { publishComment, type CommentRecord, type DiscussionRootContext } from './discussion';

  let {
    groupId,
    rootContext,
    replyingTo = undefined,
    onPublished = undefined,
    onCancelReply = undefined
  }: {
    groupId: string;
    rootContext: DiscussionRootContext;
    replyingTo?: CommentRecord | undefined;
    onPublished?: ((comment: CommentRecord) => void) | undefined;
    onCancelReply?: (() => void) | undefined;
  } = $props();

  let content = $state('');
  let publishing = $state(false);
  let errorMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  async function handleSubmit() {
    if (!content.trim() || publishing) return;

    publishing = true;
    errorMessage = '';

    try {
      await ensureClientNdk();

      const comment = await publishComment(ndk, {
        groupId,
        rootContext,
        parentComment: replyingTo,
        content: content.trim()
      });

      content = '';
      onPublished?.(comment);
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not post comment.';
    } finally {
      publishing = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void handleSubmit();
    }
  }
</script>

{#if !currentUser}
  <div class="composer-guest">
    <p>Sign in to join the discussion.</p>
  </div>
{:else if isReadOnly}
  <div class="composer-guest">
    <p>Read-only sessions cannot post comments.</p>
  </div>
{:else}
  <div class="composer">
    {#if replyingTo}
      <div class="replying-to">
        <span>Replying to {replyingTo.pubkey.slice(0, 8)}…</span>
        <button type="button" class="cancel-reply" onclick={onCancelReply}>Cancel</button>
      </div>
    {/if}

    <textarea
      bind:value={content}
      rows="3"
      maxlength="2000"
      placeholder={replyingTo ? 'Write a reply…' : 'Add a comment…'}
      onkeydown={handleKeydown}
    ></textarea>

    <div class="composer-footer">
      <button
        type="button"
        class="submit-button"
        disabled={!content.trim() || publishing}
        onclick={handleSubmit}
      >
        {publishing ? 'Posting…' : 'Post'}
      </button>

      {#if errorMessage}
        <p class="composer-error">{errorMessage}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .composer {
    display: grid;
    gap: 0.6rem;
  }

  .composer-guest {
    padding: 0.85rem 1rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1rem;
    background: var(--surface-soft);
  }

  .composer-guest p {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .replying-to {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.75rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 10%, white);
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 600;
    width: fit-content;
  }

  .cancel-reply {
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
    cursor: pointer;
    text-decoration: underline;
  }

  textarea {
    width: 100%;
    min-height: 5rem;
    padding: 0.75rem 0.9rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.85rem;
    background: white;
    color: var(--text);
    font-size: 0.9rem;
    line-height: 1.55;
    resize: vertical;
  }

  textarea:focus {
    border-color: var(--accent);
    outline: none;
  }

  .composer-footer {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .submit-button {
    display: inline-flex;
    align-items: center;
    min-height: 2.2rem;
    padding: 0 1rem;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-size: 0.84rem;
    font-weight: 700;
    cursor: pointer;
  }

  .submit-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .composer-error {
    margin: 0;
    color: #b42318;
    font-size: 0.84rem;
  }
</style>
