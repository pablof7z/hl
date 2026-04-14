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
        ? 'Highlight saved. This community already had a share for it.'
        : 'Highlight saved and shared to this community.';
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
    <p class="eyebrow">Create Highlight</p>
    <h2>Save the line that mattered.</h2>
    <p>
      Canonical `kind:9802` goes to your relay set, then Highlighter reposts it into this
      community with `kind:16`.
    </p>
  </div>

  <form class="highlight-form" onsubmit={handlePublish}>
    <label class="field">
      <span>Highlighted text</span>
      <textarea
        bind:value={quote}
        rows="5"
        maxlength="1200"
        placeholder="Paste the excerpt or quote you want to keep."
      ></textarea>
    </label>

    <label class="field">
      <span>Context</span>
      <textarea
        bind:value={context}
        rows="3"
        maxlength="500"
        placeholder="Optional surrounding text, chapter, timestamp, or location."
      ></textarea>
    </label>

    <label class="field">
      <span>Note</span>
      <textarea
        bind:value={note}
        rows="3"
        maxlength="280"
        placeholder="Optional note about why this stands out."
      ></textarea>
    </label>

    <div class="highlight-actions">
      <button class="primary" type="submit" disabled={!canPublish}>
        {publishing ? 'Saving…' : 'Save highlight'}
      </button>
      <span class="share-target">Sharing into `/community/{groupId}`</span>
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
  .highlight-form-shell {
    display: grid;
    gap: 1rem;
    padding: 1.2rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .highlight-form-copy h2 {
    margin: 0.3rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.5rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .highlight-form-copy p:last-child {
    margin: 0.55rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .highlight-form {
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

  .field textarea {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    color: var(--text);
    padding: 0.85rem 0.95rem;
    resize: vertical;
  }

  .highlight-actions {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .primary {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.75rem;
    padding: 0 1rem;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 700;
  }

  .primary:disabled {
    opacity: 0.55;
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
