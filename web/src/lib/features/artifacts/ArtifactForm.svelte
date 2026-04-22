<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    formatPodcastDuration,
    formatPodcastReleaseDate
  } from '$lib/features/podcasts/format';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import {
    artifactPath,
    publishArtifact,
    type ArtifactPreview
  } from '$lib/ndk/artifacts';
  import * as Dialog from '$lib/components/ui/dialog';

  let {
    groupId,
    open = $bindable(false)
  }: {
    groupId: string;
    open?: boolean;
  } = $props();

  let reference = $state('');
  let source = $state<'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web'>('article');
  let note = $state('');
  let preview = $state<ArtifactPreview | null>(null);
  let previewing = $state(false);
  let publishing = $state(false);
  let errorMessage = $state('');
  let statusMessage = $state('');

  $effect(() => {
    if (!open) {
      reference = '';
      source = 'article';
      note = '';
      preview = null;
      errorMessage = '';
      statusMessage = '';
    }
  });

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const canPreview = $derived(Boolean(reference.trim()) && !previewing);
  const canPublish = $derived(Boolean(preview) && !publishing && !isReadOnly);

  async function loadPreview() {
    if (!reference.trim()) {
      errorMessage = 'Paste a URL or Nostr article reference to preview it.';
      return null;
    }

    previewing = true;
    errorMessage = '';
    statusMessage = '';

    try {
      const response = await fetch('/api/artifacts/preview', {
        method: 'POST',
        headers: {
          'content-type': 'application/json'
        },
        body: JSON.stringify({ reference, source })
      });

      const body = (await response.json()) as ArtifactPreview | { error?: string };
      if (!response.ok) {
        throw new Error('error' in body && body.error ? body.error : 'Could not preview that URL.');
      }

      preview = body as ArtifactPreview;
      preview = { ...preview, source };
      reference = preview.url;
      return preview;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not preview that reference.';
      preview = null;
      return null;
    } finally {
      previewing = false;
    }
  }

  async function handlePreview(event: SubmitEvent) {
    event.preventDefault();
    await loadPreview();
  }

  async function handlePublish() {
    if (!currentUser) {
      errorMessage = 'Sign in before sharing sources.';
      return;
    }

    if (isReadOnly) {
      errorMessage = 'Read-only sessions cannot publish shares.';
      return;
    }

    const resolvedPreview = preview ?? (await loadPreview());
    if (!resolvedPreview) return;

    publishing = true;
    errorMessage = '';
    statusMessage = '';

    try {
      await ensureClientNdk();

      const result = await publishArtifact(ndk, {
        groupId,
        preview: resolvedPreview,
        note
      });

      statusMessage = result.existing
        ? 'That source is already shared in this room. Opening the existing entry.'
        : 'Content shared. Opening the detail page.';

      await goto(artifactPath(groupId, result.artifact.id), { invalidateAll: true });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not share that content.';
    } finally {
      publishing = false;
    }
  }

  function handleSourceChange(nextSource: typeof source) {
    source = nextSource;
    if (preview) {
      preview = { ...preview, source: nextSource };
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="artifact-form-dialog">
    <div class="artifact-form-chrome">
      <div class="artifact-form-handle" aria-hidden="true"></div>

      <Dialog.Header class="artifact-form-header">
        <Dialog.Title>Share a source</Dialog.Title>
      </Dialog.Header>

      <Dialog.Close class="btn btn-circle btn-ghost btn-sm" aria-label="Close share dialog">
        <svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </Dialog.Close>
    </div>

    <div class="artifact-form-body">
  <form class="artifact-form" onsubmit={handlePreview}>
    <fieldset class="fieldset">
      <legend class="fieldset-legend">Type</legend>
      <select class="field-select" bind:value={source} onchange={(event) => handleSourceChange((event.currentTarget as HTMLSelectElement).value as typeof source)}>
        <option value="article">Article</option>
        <option value="book">Book</option>
        <option value="podcast">Podcast</option>
        <option value="video">Video</option>
        <option value="paper">Paper</option>
        <option value="web">Web page</option>
      </select>
    </fieldset>

    <fieldset class="fieldset">
      <legend class="fieldset-legend">URL or Nostr article</legend>
      <input
        class="field-input"
        bind:value={reference}
        type="text"
        inputmode="url"
        placeholder="https://example.com/article or naddr1..."
        autocomplete="off"
      />
    </fieldset>

    <fieldset class="fieldset">
      <legend class="fieldset-legend">Why share it?</legend>
      <textarea
        class="field-input"
        bind:value={note}
        rows="4"
        maxlength="280"
        placeholder="Optional note for the room."
      ></textarea>
    </fieldset>

    <div class="artifact-actions">
      <button class="btn" type="submit" disabled={!canPreview}>
        {previewing ? 'Previewing…' : 'Preview'}
      </button>
      <button class="btn btn-primary" type="button" disabled={!canPublish} onclick={handlePublish}>
        {publishing ? 'Sharing…' : 'Share with room'}
      </button>
    </div>

    {#if errorMessage}
      <p class="error">{errorMessage}</p>
    {/if}

    {#if statusMessage}
      <p class="status">{statusMessage}</p>
    {/if}

    {#if preview}
      <div class="preview-card">
        {#if preview.image}
          <img src={preview.image} alt="" loading="lazy" />
        {/if}

        <div class="preview-copy">
          <div class="preview-topline">
            <span>{preview.source}</span>
            {#if preview.domain}
              <span>{preview.domain}</span>
            {/if}
          </div>
          <strong>{preview.title}</strong>
          {#if preview.podcastShowTitle}
            <p>{preview.podcastShowTitle}</p>
          {/if}
          {#if preview.author}
            <p>{preview.author}</p>
          {/if}
          {#if preview.source === 'podcast' && (preview.durationSeconds || preview.publishedAt)}
            <p>
              {#if preview.durationSeconds}
                <span>{formatPodcastDuration(preview.durationSeconds)}</span>
              {/if}
              {#if preview.durationSeconds && preview.publishedAt}
                <span> · </span>
              {/if}
              {#if preview.publishedAt}
                <span>{formatPodcastReleaseDate(preview.publishedAt)}</span>
              {/if}
            </p>
          {/if}
          {#if preview.description}
            <p>{preview.description}</p>
          {/if}
        </div>
      </div>
    {/if}
  </form>
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  .field-input {
    width: 100%;
    padding: 0.625rem 0.75rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.75rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
    transition: border-color 120ms ease;
    resize: vertical;
  }

  .field-input::placeholder { color: var(--muted); }
  .field-input:focus { border-color: var(--accent); }

  .field-select {
    width: 100%;
    padding: 0.625rem 0.75rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.75rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
    transition: border-color 120ms ease;
    cursor: pointer;
  }

  .field-select:focus { border-color: var(--accent); }

  :global(.artifact-form-dialog) {
    padding: 1.15rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      #ffffff;
  }

  .artifact-form-chrome {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: start;
    gap: 0.9rem;
  }

  .artifact-form-handle {
    grid-column: 1 / -1;
    width: 3rem;
    height: 0.3rem;
    border-radius: 999px;
    background: rgba(17, 17, 17, 0.08);
    margin: 0 auto 0.15rem;
  }

  :global(.artifact-form-header) {
    gap: 0.35rem;
  }

  .artifact-form-body {
    margin-top: 1rem;
  }

  .preview-copy p {
    margin: 0.55rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .artifact-form {
    display: grid;
    gap: 0.95rem;
  }

  .artifact-actions {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .error,
  .status {
    margin: 0;
    padding: 0.8rem 0.95rem;
    border-radius: 0.95rem;
    font-size: 0.88rem;
    line-height: 1.55;
  }

  .error {
    background: var(--pale-red);
    color: var(--pale-red-text);
  }

  .status {
    background: var(--pale-green);
    color: var(--pale-green-text);
  }

  .preview-card {
    display: grid;
    grid-template-columns: minmax(110px, 140px) minmax(0, 1fr);
    gap: 1rem;
    padding: 0.95rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1rem;
    background: var(--surface-soft);
  }

  .preview-card img {
    width: 100%;
    height: 100%;
    aspect-ratio: 4 / 5;
    object-fit: cover;
    border-radius: 0.85rem;
  }

  .preview-copy {
    display: grid;
    gap: 0.45rem;
    min-width: 0;
  }

  .preview-topline {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .preview-topline span {
    display: inline-flex;
    align-items: center;
    min-height: 1.75rem;
    padding: 0 0.55rem;
    border-radius: 999px;
    background: white;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .preview-copy strong {
    color: var(--text-strong);
    font-size: 1rem;
    line-height: 1.35;
  }

  @media (max-width: 720px) {
    .preview-card {
      grid-template-columns: 1fr;
    }

    .preview-card img {
      aspect-ratio: 16 / 9;
    }
  }
</style>
