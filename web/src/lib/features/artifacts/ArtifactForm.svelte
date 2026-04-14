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

  let {
    groupId
  }: {
    groupId: string;
  } = $props();

  let reference = $state('');
  let source = $state<'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web'>('article');
  let note = $state('');
  let preview = $state<ArtifactPreview | null>(null);
  let previewing = $state(false);
  let publishing = $state(false);
  let errorMessage = $state('');
  let statusMessage = $state('');

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
        ? 'That source is already shared in this community. Opening the existing entry.'
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

<section class="artifact-form-shell">
  <div class="artifact-form-copy">
    <h2>Share a source</h2>
  </div>

  <form class="artifact-form" onsubmit={handlePreview}>
    <label class="field">
      <span>Type</span>
      <select bind:value={source} onchange={(event) => handleSourceChange((event.currentTarget as HTMLSelectElement).value as typeof source)}>
        <option value="article">Article</option>
        <option value="book">Book</option>
        <option value="podcast">Podcast</option>
        <option value="video">Video</option>
        <option value="paper">Paper</option>
        <option value="web">Web page</option>
      </select>
    </label>

    <label class="field">
      <span>URL or Nostr article</span>
      <input
        bind:value={reference}
        type="text"
        inputmode="url"
        placeholder="https://example.com/article or naddr1..."
        autocomplete="off"
      />
    </label>

    <label class="field">
      <span>Why share it?</span>
      <textarea
        bind:value={note}
        rows="4"
        maxlength="280"
        placeholder="Optional note for the community."
      ></textarea>
    </label>

    <div class="artifact-actions">
      <button class="secondary" type="submit" disabled={!canPreview}>
        {previewing ? 'Previewing…' : 'Preview'}
      </button>
      <button class="primary" type="button" disabled={!canPublish} onclick={handlePublish}>
        {publishing ? 'Sharing…' : 'Share with community'}
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
            <span>{preview.domain}</span>
            {#if preview.catalogKind && preview.catalogKind !== 'web' && preview.catalogKind !== 'nostr:30023'}
              <span>{preview.catalogKind}</span>
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
          {#if preview.catalogId && preview.catalogKind !== 'web' && preview.catalogId !== preview.url}
            <code>{preview.catalogId}</code>
          {/if}
          <code>/community/{groupId}/content/{preview.id}</code>
        </div>
      </div>
    {/if}
  </form>
</section>

<style>
  .artifact-form-shell {
    display: grid;
    gap: 1rem;
    padding: 1.2rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .artifact-form-copy h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.5rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
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

  .field {
    display: grid;
    gap: 0.45rem;
  }

  .field span {
    color: var(--text-strong);
    font-size: 0.88rem;
    font-weight: 700;
  }

  .field input,
  .field textarea,
  .field select {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    color: var(--text);
    padding: 0.85rem 0.95rem;
  }

  .field textarea {
    min-height: 6rem;
    resize: vertical;
  }

  .artifact-actions {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .artifact-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.8rem;
    padding: 0 1rem;
    border-radius: 999px;
    font-weight: 700;
    cursor: pointer;
  }

  .artifact-actions button.secondary {
    border: 1px solid var(--border);
    background: white;
    color: var(--text-strong);
  }

  .artifact-actions button.primary {
    border: 0;
    background: var(--accent);
    color: white;
  }

  .artifact-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
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
    border: 1px solid var(--border);
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

  .preview-topline span,
  .preview-copy code {
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
