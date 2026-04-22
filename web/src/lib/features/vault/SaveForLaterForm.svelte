<script lang="ts">
  import {
    formatPodcastDuration,
    formatPodcastReleaseDate
  } from '$lib/features/podcasts/format';
  import type { ArtifactPreview } from '$lib/ndk/artifacts';
  import { saveForLaterArtifact, type ForLaterItem } from './vault';

  let {
    onSaved = undefined
  }: {
    onSaved?: ((item: ForLaterItem) => void) | undefined;
  } = $props();

  let reference = $state('');
  let source = $state<'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web'>('article');
  let preview = $state<ArtifactPreview | null>(null);
  let previewing = $state(false);
  let saving = $state(false);
  let errorMessage = $state('');
  let statusMessage = $state('');

  const canPreview = $derived(Boolean(reference.trim()) && !previewing);
  const canSave = $derived(Boolean(preview) && !saving);

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
        throw new Error('error' in body && body.error ? body.error : 'Could not preview that reference.');
      }

      preview = { ...(body as ArtifactPreview), source };
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

  async function handleSave() {
    const resolvedPreview = preview ?? (await loadPreview());
    if (!resolvedPreview) return;

    saving = true;
    errorMessage = '';
    statusMessage = '';

    try {
      const result = await saveForLaterArtifact({
        artifact: resolvedPreview
      });

      statusMessage = result.existing
        ? 'Already in your NIP-51 bookmark list.'
        : 'Saved to your NIP-51 bookmark list.';
      onSaved?.(result.item);

      reference = '';
      preview = null;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not save this item for later.';
    } finally {
      saving = false;
    }
  }

  function handleSourceChange(nextSource: typeof source) {
    source = nextSource;
    if (preview) {
      preview = { ...preview, source: nextSource };
    }
  }
</script>

<section class="save-form-shell">
  <div class="save-form-copy">
    <h2>Save a source</h2>
    <p>Store it as a standard NIP-51 bookmark tag.</p>
  </div>

  <form class="save-form" onsubmit={handlePreview}>
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

    <div class="save-actions">
      <button class="btn" type="submit" disabled={!canPreview}>
        {previewing ? 'Previewing…' : 'Preview'}
      </button>
      <button class="btn btn-primary" type="button" disabled={!canSave} onclick={handleSave}>
        {saving ? 'Saving…' : 'Save to For Later'}
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
</section>

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

  .save-form-shell {
    display: grid;
    gap: 1rem;
    padding: 1.2rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .save-form-copy h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.5rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .save-form-copy p {
    margin: 0.35rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .preview-copy p {
    margin: 0.55rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .save-form {
    display: grid;
    gap: 0.95rem;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .save-actions {
    grid-column: 1 / -1;
    display: flex;
    gap: 0.65rem;
    flex-wrap: wrap;
  }

  .error,
  .status {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .error {
    color: #b42318;
  }

  .status {
    color: var(--muted);
  }

  .preview-card {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: minmax(120px, 160px) minmax(0, 1fr);
    gap: 1rem;
    padding: 1rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.05rem;
    background: white;
  }

  .preview-card img {
    width: 100%;
    height: 100%;
    min-height: 10rem;
    object-fit: cover;
    border-radius: 0.9rem;
  }

  .preview-copy {
    display: grid;
    gap: 0.45rem;
  }

  .preview-copy strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    line-height: 1.2;
  }

  .preview-topline {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .preview-topline span {
    display: inline-flex;
    align-items: center;
    min-height: 1.75rem;
    padding: 0 0.55rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.74rem;
    font-weight: 700;
  }

  @media (max-width: 760px) {
    .save-form {
      grid-template-columns: 1fr;
    }

    .preview-card {
      grid-template-columns: 1fr;
    }
  }
</style>
