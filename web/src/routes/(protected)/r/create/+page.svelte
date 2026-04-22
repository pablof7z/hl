<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';
  import {
    createCommunity,
    slugifyCommunityId,
    type CommunityAccess,
    type CommunityVisibility
  } from '$lib/ndk/groups';

  let name = $state('');
  let communityId = $state('');
  let communityIdTouched = $state(false);
  let about = $state('');
  let picture = $state('');
  let access = $state<CommunityAccess>('open');
  let visibility = $state<CommunityVisibility>('public');
  let saving = $state(false);
  let errorMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const normalizedCommunityId = $derived(slugifyCommunityId(communityId));
  const canSubmit = $derived(
    Boolean(name.trim()) && normalizedCommunityId.length >= 3 && !saving && !isReadOnly
  );

  $effect(() => {
    if (communityIdTouched) return;
    communityId = slugifyCommunityId(name);
  });

  $effect(() => {
    if (visibility === 'private' && access === 'open') {
      access = 'closed';
    }
  });

  function handleCommunityIdInput(event: Event) {
    communityIdTouched = true;
    communityId = slugifyCommunityId((event.currentTarget as HTMLInputElement).value);
  }

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();

    if (!currentUser) {
      errorMessage = 'Sign in before creating a room.';
      return;
    }

    if (!canSubmit) {
      errorMessage = isReadOnly
        ? 'Read-only sessions cannot create communities.'
        : 'Enter a name and a valid room URL.';
      return;
    }

    try {
      saving = true;
      errorMessage = '';

      await ensureClientNdk();

      const result = await createCommunity(ndk, {
        id: normalizedCommunityId,
        name,
        about,
        picture,
        access,
        visibility
      });

      await goto(`/r/${result.id}`, { invalidateAll: true });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not create the room.';
    } finally {
      saving = false;
    }
  }
</script>

<svelte:head>
  <title>Create Room — Highlighter</title>
</svelte:head>

<section class="community-create">
  <header class="community-create-header">
    <div>
      <h1>Create a room</h1>
    </div>

    <div class="relay-note">
      <span>Relay</span>
      <strong>{HIGHLIGHTER_RELAY_URL}</strong>
    </div>
  </header>

  <form class="community-form" onsubmit={handleSubmit}>
    <section class="form-card">
      <fieldset class="fieldset">
        <legend class="fieldset-legend">Name</legend>
        <input
          class="input w-full"
          bind:value={name}
          placeholder="Signal over noise"
          maxlength="80"
          autocomplete="off"
        />
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Room URL</legend>
        <div class="slug-input">
          <span>/r/</span>
          <input
            value={communityId}
            oninput={handleCommunityIdInput}
            placeholder="signal-over-noise"
            maxlength="48"
            autocomplete="off"
          />
        </div>
        <p class="fieldset-label">Lowercase letters, numbers, and hyphens only.</p>
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Description</legend>
        <textarea
          class="textarea w-full"
          bind:value={about}
          rows="5"
          maxlength="280"
          placeholder="What kind of reading and conversation belongs here?"
        ></textarea>
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Cover image URL</legend>
        <input
          class="input w-full"
          bind:value={picture}
          placeholder="https://..."
          inputmode="url"
          autocomplete="off"
        />
      </fieldset>
    </section>

    <section class="form-card">
      <fieldset class="fieldset">
        <legend class="fieldset-legend">Access</legend>
        <div class="option-row">
          <label class:active={access === 'open'}>
            <input type="radio" bind:group={access} value="open" disabled={visibility === 'private'} />
            <strong>Open</strong>
            <small>Anyone can join without an invite.</small>
          </label>

          <label class:active={access === 'closed'}>
            <input type="radio" bind:group={access} value="closed" />
            <strong>Closed</strong>
            <small>Membership requires approval or invite codes.</small>
          </label>
        </div>
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Visibility</legend>
        <div class="option-row">
          <label class:active={visibility === 'public'}>
            <input type="radio" bind:group={visibility} value="public" />
            <strong>Public</strong>
            <small>Room metadata can be browsed openly.</small>
          </label>

          <label class:active={visibility === 'private'}>
            <input type="radio" bind:group={visibility} value="private" />
            <strong>Private</strong>
            <small>Content is members-only and forces closed membership.</small>
          </label>
        </div>
      </fieldset>

      <div class="preview-card">
        <p class="preview-label">Preview</p>
        <p class="preview-route">/r/{normalizedCommunityId || 'your-room'}</p>
        <p class="preview-copy">
          {about || 'Add a short description so people know what belongs here.'}
        </p>
      </div>

      {#if errorMessage}
        <p class="error-message">{errorMessage}</p>
      {/if}

      {#if isReadOnly}
        <p class="read-only-note">
          This signer is read-only. Switch to a writable signer to create a room.
        </p>
      {/if}

      <button class="btn btn-primary" type="submit" disabled={!canSubmit}>
        {saving ? 'Publishing…' : 'Create room'}
      </button>
    </section>
  </form>
</section>

<style>
  .community-create {
    display: grid;
    gap: 1.5rem;
    padding: 2rem 0 3rem;
  }

  .community-create-header {
    display: flex;
    justify-content: space-between;
    align-items: end;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .preview-label,
  .relay-note span {
    margin: 0 0 0.45rem;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .relay-note {
    display: grid;
    gap: 0.15rem;
    padding: 0.9rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface-soft);
  }

  .relay-note strong {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    color: var(--text-strong);
  }

  .community-form {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.9fr);
    gap: 1rem;
    align-items: start;
  }

  .form-card {
    display: grid;
    gap: 1rem;
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background: var(--surface);
  }

  .option-row small {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .slug-input {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    overflow: hidden;
  }

  .slug-input span {
    display: inline-flex;
    align-items: center;
    min-height: 100%;
    padding: 0 0.85rem;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.84rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .slug-input input {
    border: 0;
    border-radius: 0;
  }

  .option-row {
    display: grid;
    gap: 0.65rem;
  }

  .option-row label {
    display: grid;
    gap: 0.25rem;
    padding: 0.95rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface-soft);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .option-row label.active {
    border-color: rgba(255, 103, 25, 0.32);
    background: rgba(255, 103, 25, 0.05);
  }

  .option-row label:has(input:disabled) {
    opacity: 0.65;
  }

  .option-row input {
    margin: 0;
  }

  .preview-card {
    padding: 1rem;
    border-radius: 1rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.1), transparent 42%),
      var(--surface-soft);
  }

  .preview-route {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-mono);
    font-size: 0.92rem;
  }

  .preview-copy {
    margin: 0.65rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .error-message,
  .read-only-note {
    margin: 0;
    padding: 0.8rem 0.95rem;
    border-radius: 0.95rem;
    font-size: 0.88rem;
    line-height: 1.55;
  }

  .error-message {
    background: var(--pale-red);
    color: var(--pale-red-text);
  }

  .read-only-note {
    background: var(--pale-yellow);
    color: var(--pale-yellow-text);
  }

  @media (max-width: 860px) {
    .community-form {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 720px) {
    .community-create {
      padding-top: 1.5rem;
    }

    .slug-input {
      grid-template-columns: 1fr;
    }

    .slug-input span {
      padding-top: 0.65rem;
      padding-bottom: 0.2rem;
      background: transparent;
    }
  }
</style>
