<script lang="ts">
  import { goto } from '$app/navigation';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import * as Dialog from '$lib/components/ui/dialog';
  import type { ArtifactPreview } from '$lib/ndk/artifacts';
  import { publishRoomDiscussion, discussionPath } from './roomDiscussion';

  let {
    groupId,
    open = $bindable(false)
  }: {
    groupId: string;
    open?: boolean;
  } = $props();

  let title = $state('');
  let body = $state('');
  let attachReference = $state('');
  let attachSource = $state<'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web'>('article');
  let attachment = $state<ArtifactPreview | null>(null);
  let attachOpen = $state(false);
  let previewing = $state(false);
  let publishing = $state(false);
  let errorMessage = $state('');

  $effect(() => {
    if (!open) {
      title = '';
      body = '';
      attachReference = '';
      attachSource = 'article';
      attachment = null;
      attachOpen = false;
      errorMessage = '';
    }
  });

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const canPublish = $derived(
    Boolean(title.trim()) && !publishing && !isReadOnly && Boolean(currentUser)
  );

  async function loadPreview() {
    if (!attachReference.trim()) {
      errorMessage = 'Paste a link or Nostr reference to attach.';
      return;
    }

    previewing = true;
    errorMessage = '';

    try {
      const response = await fetch('/api/artifacts/preview', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ reference: attachReference, source: attachSource })
      });

      const payload = (await response.json()) as ArtifactPreview | { error?: string };
      if (!response.ok) {
        throw new Error('error' in payload && payload.error ? payload.error : 'Could not preview that link.');
      }

      attachment = { ...(payload as ArtifactPreview), source: attachSource };
      attachReference = attachment.url;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not preview that link.';
      attachment = null;
    } finally {
      previewing = false;
    }
  }

  function clearAttachment() {
    attachment = null;
    attachReference = '';
  }

  async function handlePublish() {
    if (!canPublish) return;

    publishing = true;
    errorMessage = '';

    try {
      await ensureClientNdk();

      const record = await publishRoomDiscussion(ndk, {
        groupId,
        title: title.trim(),
        body: body.trim(),
        attachment
      });

      open = false;
      await goto(discussionPath(groupId, record.id));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not publish the discussion.';
    } finally {
      publishing = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="discussion-composer-dialog">
    <div class="dc-chrome">
      <div class="dc-handle" aria-hidden="true"></div>
      <Dialog.Header class="dc-header">
        <Dialog.Title>Start a discussion</Dialog.Title>
        <Dialog.Description class="dc-sub">
          Propose a read, unpack an idea, or ask the room a question.
        </Dialog.Description>
      </Dialog.Header>
      <Dialog.Close class="btn btn-circle btn-ghost btn-sm dc-close" aria-label="Close composer">
        <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </Dialog.Close>
    </div>

    <div class="dc-body">
      <label class="dc-title-field">
        <span class="dc-label">Title</span>
        <input
          class="dc-title-input"
          type="text"
          maxlength="200"
          placeholder="A book, a thought, a question…"
          bind:value={title}
        />
      </label>

      <label class="dc-body-field">
        <span class="dc-label">Your take</span>
        <textarea
          class="dc-body-input"
          rows="6"
          maxlength="6000"
          placeholder="Why does it matter? What do you want the room to weigh in on?"
          bind:value={body}
        ></textarea>
      </label>

      {#if attachment}
        <div class="dc-attached">
          <div class="dc-attached-head">
            <span class="dc-attached-tag">Attached · {attachment.source}</span>
            <button type="button" class="dc-attached-clear" onclick={clearAttachment}>Remove</button>
          </div>
          <div class="dc-attached-card">
            {#if attachment.image}
              <img src={attachment.image} alt="" loading="lazy" />
            {/if}
            <div class="dc-attached-copy">
              <strong>{attachment.title}</strong>
              {#if attachment.author}<p class="dc-attached-meta">{attachment.author}</p>{/if}
              {#if attachment.domain}<p class="dc-attached-meta">{attachment.domain}</p>{/if}
              {#if attachment.description}<p class="dc-attached-desc">{attachment.description}</p>{/if}
            </div>
          </div>
        </div>
      {:else if attachOpen}
        <div class="dc-attach-panel">
          <div class="dc-attach-row">
            <select class="dc-attach-select" bind:value={attachSource}>
              <option value="article">Article</option>
              <option value="book">Book</option>
              <option value="podcast">Podcast</option>
              <option value="video">Video</option>
              <option value="paper">Paper</option>
              <option value="web">Web</option>
            </select>
            <input
              class="dc-attach-input"
              type="text"
              inputmode="url"
              placeholder="https://…  or  naddr1…"
              bind:value={attachReference}
            />
          </div>
          <div class="dc-attach-actions">
            <button type="button" class="btn btn-sm" onclick={loadPreview} disabled={previewing || !attachReference.trim()}>
              {previewing ? 'Fetching…' : 'Attach'}
            </button>
            <button type="button" class="btn btn-ghost btn-sm" onclick={() => { attachOpen = false; attachReference = ''; }}>
              Cancel
            </button>
          </div>
        </div>
      {:else}
        <button type="button" class="dc-attach-cta" onclick={() => (attachOpen = true)}>
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12.3V7a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v10a4 4 0 0 0 4 4h6" />
            <path d="M14 12.5L9.5 17l-2.5-2.5" />
          </svg>
          Attach a book, article, podcast…
        </button>
      {/if}

      {#if errorMessage}
        <p class="dc-error">{errorMessage}</p>
      {/if}

      <div class="dc-footer">
        <span class="dc-footer-hint">
          {#if !currentUser}
            Sign in to post.
          {:else if isReadOnly}
            Read-only session.
          {:else}
            Visible to everyone in this room.
          {/if}
        </span>
        <button
          type="button"
          class="btn btn-primary"
          disabled={!canPublish}
          onclick={handlePublish}
        >
          {publishing ? 'Publishing…' : 'Publish discussion'}
        </button>
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.discussion-composer-dialog) {
    padding: 1.15rem;
    max-width: 640px;
    background:
      radial-gradient(circle at top left, rgba(194, 77, 44, 0.09), transparent 40%),
      #ffffff;
  }

  .dc-chrome {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: start;
    gap: 0.9rem;
  }

  .dc-handle {
    grid-column: 1 / -1;
    width: 3rem;
    height: 0.3rem;
    border-radius: 999px;
    background: rgba(17, 17, 17, 0.08);
    margin: 0 auto 0.15rem;
  }

  :global(.dc-header) {
    gap: 0.25rem;
  }

  :global(.dc-sub) {
    font-size: 0.85rem;
    color: var(--muted);
  }

  :global(.dc-close) { align-self: start; }

  .dc-body {
    margin-top: 0.9rem;
    display: grid;
    gap: 1rem;
  }

  .dc-label {
    display: block;
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-fade, #8a8378);
    margin-bottom: 0.4rem;
    font-weight: 600;
  }

  .dc-title-input {
    width: 100%;
    padding: 0.55rem 0.2rem;
    border: 0;
    border-bottom: 1px solid var(--color-base-300);
    background: transparent;
    color: var(--ink, #15130F);
    font-family: var(--font-serif, Georgia, serif);
    font-size: 1.55rem;
    line-height: 1.2;
    letter-spacing: -0.01em;
    outline: none;
    transition: border-color 150ms ease;
  }

  .dc-title-input::placeholder {
    color: var(--ink-fade, #a8a095);
    font-style: italic;
  }

  .dc-title-input:focus {
    border-bottom-color: var(--brand-accent, #C24D2C);
  }

  .dc-body-input {
    width: 100%;
    padding: 0.7rem 0.85rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.85rem;
    background: var(--surface-soft, #FAF8F3);
    color: var(--ink, #15130F);
    font-family: inherit;
    font-size: 0.95rem;
    line-height: 1.55;
    outline: none;
    resize: vertical;
    min-height: 7rem;
    transition: border-color 150ms ease, background 150ms ease;
  }

  .dc-body-input:focus {
    border-color: var(--brand-accent, #C24D2C);
    background: #fff;
  }

  .dc-attach-cta {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.85rem;
    border: 1px dashed var(--color-base-300);
    border-radius: 999px;
    background: transparent;
    color: var(--ink-fade, #8a8378);
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    width: fit-content;
    transition: color 150ms ease, border-color 150ms ease, background 150ms ease;
  }

  .dc-attach-cta:hover {
    color: var(--brand-accent, #C24D2C);
    border-color: var(--brand-accent, #C24D2C);
    background: color-mix(in srgb, var(--brand-accent, #C24D2C) 6%, white);
  }

  .dc-attach-panel {
    display: grid;
    gap: 0.55rem;
    padding: 0.85rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.85rem;
    background: var(--surface-soft, #FAF8F3);
  }

  .dc-attach-row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 0.5rem;
  }

  .dc-attach-select,
  .dc-attach-input {
    padding: 0.55rem 0.7rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.65rem;
    background: #fff;
    font: inherit;
    font-size: 0.88rem;
    outline: none;
    transition: border-color 150ms ease;
  }

  .dc-attach-select:focus,
  .dc-attach-input:focus { border-color: var(--brand-accent, #C24D2C); }

  .dc-attach-actions {
    display: flex;
    gap: 0.45rem;
  }

  .dc-attached {
    display: grid;
    gap: 0.5rem;
  }

  .dc-attached-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .dc-attached-tag {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--brand-accent, #C24D2C);
    font-weight: 600;
  }

  .dc-attached-clear {
    margin-left: auto;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted, #8a8378);
    font-size: 0.78rem;
    text-decoration: underline;
    cursor: pointer;
  }

  .dc-attached-card {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    gap: 0.85rem;
    padding: 0.75rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.85rem;
    background: #fff;
  }

  .dc-attached-card img {
    width: 100%;
    height: 100%;
    aspect-ratio: 4 / 5;
    object-fit: cover;
    border-radius: 0.55rem;
  }

  .dc-attached-copy strong {
    display: block;
    font-family: var(--font-serif, Georgia, serif);
    font-size: 1rem;
    line-height: 1.3;
    color: var(--ink, #15130F);
  }

  .dc-attached-meta {
    margin: 0.3rem 0 0;
    color: var(--muted, #8a8378);
    font-size: 0.8rem;
  }

  .dc-attached-desc {
    margin: 0.4rem 0 0;
    font-size: 0.82rem;
    line-height: 1.5;
    color: var(--ink-soft, #55514a);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .dc-error {
    margin: 0;
    padding: 0.7rem 0.85rem;
    border-radius: 0.75rem;
    background: var(--pale-red, #fdecec);
    color: var(--pale-red-text, #b42318);
    font-size: 0.85rem;
  }

  .dc-footer {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding-top: 0.3rem;
    border-top: 1px solid var(--color-base-300);
    padding: 0.85rem 0 0;
    flex-wrap: wrap;
  }

  .dc-footer-hint {
    color: var(--muted, #8a8378);
    font-size: 0.8rem;
  }

  .dc-footer .btn {
    margin-left: auto;
  }

  @media (max-width: 560px) {
    .dc-attach-row { grid-template-columns: 1fr; }
    .dc-attached-card { grid-template-columns: 1fr; }
    .dc-attached-card img { aspect-ratio: 16 / 9; }
  }
</style>
