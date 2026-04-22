<script lang="ts">
  import { goto } from '$app/navigation';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import type { ArtifactPreview } from '$lib/ndk/artifacts';
  import { publishRoomDiscussion, discussionPath } from './roomDiscussion';

  let {
    groupId,
    onCancel,
    onPublished
  }: {
    groupId: string;
    onCancel?: () => void;
    onPublished?: () => void;
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

      onPublished?.();
      await goto(discussionPath(groupId, record.id));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not publish the discussion.';
    } finally {
      publishing = false;
    }
  }
</script>

<section class="composer">
  <input
    class="input input-bordered w-full title-input"
    type="text"
    maxlength="200"
    placeholder="Title"
    bind:value={title}
  />

  <textarea
    class="textarea textarea-bordered w-full"
    rows="8"
    maxlength="6000"
    placeholder="Text (optional)"
    bind:value={body}
  ></textarea>

  {#if attachment}
    <div class="attached">
      <div class="attached-head">
        <span class="attached-tag">{attachment.source}</span>
        <button type="button" class="btn btn-ghost btn-xs" onclick={clearAttachment}>Remove</button>
      </div>
      <div class="attached-card">
        {#if attachment.image}
          <img src={attachment.image} alt="" loading="lazy" />
        {/if}
        <div class="attached-copy">
          <strong>{attachment.title}</strong>
          {#if attachment.author}<p>{attachment.author}</p>{/if}
          {#if attachment.domain}<p class="muted">{attachment.domain}</p>{/if}
        </div>
      </div>
    </div>
  {:else if attachOpen}
    <div class="attach-panel">
      <div class="attach-row">
        <select class="select select-bordered select-sm" bind:value={attachSource}>
          <option value="article">Article</option>
          <option value="book">Book</option>
          <option value="podcast">Podcast</option>
          <option value="video">Video</option>
          <option value="paper">Paper</option>
          <option value="web">Web</option>
        </select>
        <input
          class="input input-bordered input-sm"
          type="text"
          inputmode="url"
          placeholder="https://…  or  naddr1…"
          bind:value={attachReference}
        />
      </div>
      <div class="attach-actions">
        <button type="button" class="btn btn-sm" onclick={loadPreview} disabled={previewing || !attachReference.trim()}>
          {previewing ? 'Fetching…' : 'Attach'}
        </button>
        <button type="button" class="btn btn-ghost btn-sm" onclick={() => { attachOpen = false; attachReference = ''; }}>
          Cancel
        </button>
      </div>
    </div>
  {:else}
    <button type="button" class="btn btn-ghost btn-sm attach-btn" onclick={() => (attachOpen = true)}>
      + Attach a link
    </button>
  {/if}

  {#if errorMessage}
    <p class="error">{errorMessage}</p>
  {/if}

  <footer class="composer-footer">
    {#if !currentUser}
      <span class="hint">Sign in to post.</span>
    {:else if isReadOnly}
      <span class="hint">Read-only session.</span>
    {/if}
    <div class="footer-actions">
      {#if onCancel}
        <button type="button" class="btn btn-ghost" onclick={onCancel}>Cancel</button>
      {/if}
      <button
        type="button"
        class="btn btn-primary"
        disabled={!canPublish}
        onclick={handlePublish}
      >
        {publishing ? 'Publishing…' : 'Publish'}
      </button>
    </div>
  </footer>
</section>

<style>
  .composer {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .title-input {
    font-size: 18px;
    font-weight: 600;
  }

  .attach-btn {
    align-self: flex-start;
  }

  .attach-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--color-base-300, #e3ddd2);
    border-radius: 6px;
    background: var(--surface-soft, #faf8f3);
  }

  .attach-row {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 8px;
  }

  .attach-actions {
    display: flex;
    gap: 8px;
  }

  .attached {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .attached-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .attached-tag {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--ink-fade, #8a8378);
  }

  .attached-head .btn {
    margin-left: auto;
  }

  .attached-card {
    display: grid;
    grid-template-columns: 80px minmax(0, 1fr);
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--color-base-300, #e3ddd2);
    border-radius: 6px;
    background: var(--surface-soft, #faf8f3);
  }

  .attached-card img {
    width: 100%;
    aspect-ratio: 4 / 5;
    object-fit: cover;
    border-radius: 4px;
  }

  .attached-copy strong {
    display: block;
    font-size: 14px;
    font-weight: 600;
    color: var(--ink, #15130F);
    line-height: 1.3;
  }

  .attached-copy p {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--ink-soft, #55514a);
  }

  .attached-copy p.muted {
    color: var(--ink-fade, #8a8378);
  }

  .error {
    margin: 0;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--pale-red, #fdecec);
    color: var(--pale-red-text, #b42318);
    font-size: 13px;
  }

  .composer-footer {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--color-base-300, #e3ddd2);
  }

  .hint {
    font-size: 12px;
    color: var(--ink-fade, #8a8378);
  }

  .footer-actions {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }

  @media (max-width: 560px) {
    .attach-row { grid-template-columns: 1fr; }
    .attached-card { grid-template-columns: 1fr; }
    .attached-card img { aspect-ratio: 16 / 9; }
  }
</style>
