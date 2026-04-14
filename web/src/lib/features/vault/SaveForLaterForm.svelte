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
  let teaser = $state('');
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
        artifact: resolvedPreview,
        teaser
      });

      statusMessage = result.existing
        ? 'Updated the saved item in your queue.'
        : 'Saved to your For Later queue.';
      onSaved?.(result.item);

      reference = '';
      teaser = '';
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
  </div>

  <form class="save-form" onsubmit={handlePreview}>
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

    <label class="field field-wide">
      <span>Teaser</span>
      <textarea
        bind:value={teaser}
        rows="3"
        maxlength="280"
        placeholder="Optional note for your future self."
      ></textarea>
    </label>

    <div class="save-actions">
      <button class="secondary" type="submit" disabled={!canPreview}>
        {previewing ? 'Previewing…' : 'Preview'}
      </button>
      <button class="primary" type="button" disabled={!canSave} onclick={handleSave}>
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
  .save-form-shell {
    display: grid;
    gap: 1rem;
    padding: 1.2rem;
    border: 1px solid var(--border);
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

  .field,
  .field-wide {
    display: grid;
    gap: 0.45rem;
  }

  .field-wide {
    grid-column: 1 / -1;
  }

  .field span,
  .field-wide span {
    color: var(--text-strong);
    font-size: 0.88rem;
    font-weight: 700;
  }

  .field input,
  .field textarea,
  .field select,
  .field-wide textarea {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    color: var(--text);
    padding: 0.85rem 0.95rem;
  }

  .field-wide textarea {
    min-height: 5.5rem;
    resize: vertical;
  }

  .save-actions {
    grid-column: 1 / -1;
    display: flex;
    gap: 0.65rem;
    flex-wrap: wrap;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.85rem;
    padding: 0 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    font-weight: 600;
    cursor: pointer;
  }

  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  button.secondary {
    background: var(--surface);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
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
    border: 1px solid var(--border);
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
