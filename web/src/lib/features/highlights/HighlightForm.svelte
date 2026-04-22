<script lang="ts">
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { publishAndShareHighlight } from '$lib/ndk/highlights';

  let {
    artifact,
    groupId
  }: {
    artifact: ArtifactRecord;
    groupId: string;
  } = $props();

  let quote = $state('');
  let context = $state('');
  let note = $state('');
  let publishing = $state(false);
  let errorMessage = $state('');
  let statusMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const canPublish = $derived(Boolean(quote.trim()) && !publishing && !isReadOnly);

  async function handlePublish(event: SubmitEvent) {
    event.preventDefault();

    if (!currentUser) {
      errorMessage = 'Sign in before creating highlights.';
      return;
    }

    if (isReadOnly) {
      errorMessage = 'Read-only sessions cannot publish highlight events.';
      return;
    }

    publishing = true;
    errorMessage = '';
    statusMessage = '';

    try {
      await ensureClientNdk();

      const result = await publishAndShareHighlight(ndk, {
        groupId,
        artifact,
        quote,
        context,
        note
      });

      statusMessage = result.shareExisting
        ? 'Highlight saved. This room already had a share for it.'
        : 'Highlight saved and shared to this room.';
      quote = '';
      context = '';
      note = '';
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not publish the highlight.';
    } finally {
      publishing = false;
    }
  }
</script>

<section class="highlight-form-shell">
  <div class="highlight-form-copy">
    <h2>Save a highlight</h2>
  </div>

  <form class="highlight-form" onsubmit={handlePublish}>
    <fieldset class="fieldset">
      <legend class="fieldset-legend">Highlighted text</legend>
      <textarea
        class="field-input"
        bind:value={quote}
        rows="5"
        maxlength="1200"
        placeholder="Paste the excerpt or quote you want to keep."
      ></textarea>
    </fieldset>

    <fieldset class="fieldset">
      <legend class="fieldset-legend">Context</legend>
      <textarea
        class="field-input"
        bind:value={context}
        rows="3"
        maxlength="500"
        placeholder="Optional surrounding text, chapter, timestamp, or location."
      ></textarea>
    </fieldset>

    <fieldset class="fieldset">
      <legend class="fieldset-legend">Note</legend>
      <textarea
        class="field-input"
        bind:value={note}
        rows="3"
        maxlength="280"
        placeholder="Optional note about why this stands out."
      ></textarea>
    </fieldset>

    <div class="highlight-actions">
      <button class="btn btn-primary" type="submit" disabled={!canPublish}>
        {publishing ? 'Saving…' : 'Save highlight'}
      </button>
      <span class="share-target">Sharing into `/r/{groupId}`</span>
    </div>

    {#if errorMessage}
      <p class="error">{errorMessage}</p>
    {/if}

    {#if statusMessage}
      <p class="status">{statusMessage}</p>
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

  .highlight-form-shell {
    display: grid;
    gap: 1rem;
    padding: 1.2rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .highlight-form-copy h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.5rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .highlight-form {
    display: grid;
    gap: 0.95rem;
  }

  .highlight-actions {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .share-target,
  .error,
  .status {
    margin: 0;
    font-size: 0.88rem;
    line-height: 1.55;
  }

  .share-target {
    color: var(--muted);
    font-family: var(--font-mono);
  }

  .error {
    color: #b42318;
  }

  .status {
    color: #0f766e;
  }
</style>
