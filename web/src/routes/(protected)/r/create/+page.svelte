<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import {
    createCommunity,
    slugifyCommunityId,
    isValidCommunityId,
    type CommunityAccess,
    type CommunityVisibility
  } from '$lib/ndk/groups';

  type Preset = 'invite' | 'open' | 'members';

  const PRESET_MAP: Record<Preset, { access: CommunityAccess; visibility: CommunityVisibility }> = {
    invite: { access: 'closed', visibility: 'public' },
    open: { access: 'open', visibility: 'public' },
    members: { access: 'closed', visibility: 'private' }
  };

  let step = $state<1 | 2 | 3>(1);

  let name = $state('');
  let communityId = $state('');
  let communityIdEdited = $state(false);
  let preset = $state<Preset>('invite');
  let about = $state('');
  let picture = $state('');

  let publishing = $state(false);
  let errorMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const slug = $derived(slugifyCommunityId(communityId));
  const slugIsValid = $derived(isValidCommunityId(slug));
  const step1Complete = $derived(name.trim().length > 0 && slugIsValid);

  $effect(() => {
    if (!communityIdEdited) {
      communityId = slugifyCommunityId(name);
    }
  });

  function handleSlugInput(event: Event) {
    communityIdEdited = true;
    communityId = slugifyCommunityId((event.currentTarget as HTMLInputElement).value);
  }

  function goNext() {
    errorMessage = '';
    if (step === 1) {
      if (!step1Complete) {
        errorMessage = 'Give the room a name first.';
        return;
      }
      step = 2;
      return;
    }
    if (step === 2) {
      step = 3;
      return;
    }
  }

  function goBack() {
    errorMessage = '';
    if (step === 2) step = 1;
    else if (step === 3) step = 2;
  }

  async function publishRoom() {
    if (!currentUser) {
      errorMessage = 'Sign in before creating a room.';
      return;
    }
    if (isReadOnly) {
      errorMessage = 'Read-only sessions cannot create rooms.';
      return;
    }
    if (!step1Complete) {
      errorMessage = 'The room needs a name and a valid URL.';
      step = 1;
      return;
    }

    try {
      publishing = true;
      errorMessage = '';
      await ensureClientNdk();
      const { access, visibility } = PRESET_MAP[preset];
      const result = await createCommunity(ndk, {
        id: slug,
        name,
        about,
        picture,
        access,
        visibility
      });
      await goto(`/r/${result.id}/invite?fresh=1`, { invalidateAll: true });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not create the room.';
    } finally {
      publishing = false;
    }
  }
</script>

<svelte:head>
  <title>New room — Highlighter</title>
</svelte:head>

<section class="wizard">
  <header class="wizard-head">
    <div class="wizard-step">0{step} / 03</div>
    <div class="wizard-dots" role="tablist" aria-label="Wizard progress">
      {#each [1, 2, 3] as s}
        <span class="dot" class:active={step === s} class:done={step > s} aria-hidden="true"></span>
      {/each}
    </div>
  </header>

  {#if step === 1}
    <div class="pane">
      <h1 class="pane-head">What do you want to call it?</h1>

      <label class="big-field">
        <span class="big-label">Room name</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="text"
          bind:value={name}
          placeholder="Signal over noise"
          maxlength="80"
          autocomplete="off"
          autofocus
        />
      </label>

      <div class="slug-row">
        <span class="slug-prefix">highlighter.com/r/</span>
        <input
          class="slug-input"
          value={communityId}
          oninput={handleSlugInput}
          placeholder="signal-over-noise"
          maxlength="48"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      {#if communityId && !slugIsValid}
        <p class="slug-warn">Use 3–48 lowercase letters, numbers, and hyphens.</p>
      {:else}
        <p class="slug-hint">Lowercase letters, numbers, and hyphens. The room's address.</p>
      {/if}
    </div>
  {/if}

  {#if step === 2}
    <div class="pane">
      <h1 class="pane-head">Who can read and join?</h1>

      <div class="presets">
        <label class="preset" class:active={preset === 'invite'}>
          <input type="radio" bind:group={preset} value="invite" />
          <div class="preset-body">
            <strong>By invitation</strong>
            <p>Only people you invite can join. Anyone with the link can read along.</p>
          </div>
        </label>

        <label class="preset" class:active={preset === 'open'}>
          <input type="radio" bind:group={preset} value="open" />
          <div class="preset-body">
            <strong>Open to anyone</strong>
            <p>Anyone can join. Anyone can read.</p>
          </div>
        </label>

        <label class="preset" class:active={preset === 'members'}>
          <input type="radio" bind:group={preset} value="members" />
          <div class="preset-body">
            <strong>Members only</strong>
            <p>Only members can join. Only members can see what's inside.</p>
          </div>
        </label>
      </div>

      <p class="pane-note">You can change this later in room settings.</p>
    </div>
  {/if}

  {#if step === 3}
    <div class="pane">
      <h1 class="pane-head">Describe it.</h1>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">What's this room about?</legend>
        <textarea
          class="field-input"
          bind:value={about}
          rows="4"
          maxlength="280"
          placeholder="Essays, books, and podcasts we keep coming back to."
        ></textarea>
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Cover image URL</legend>
        <input
          class="field-input"
          bind:value={picture}
          placeholder="https://..."
          inputmode="url"
          autocomplete="off"
        />
      </fieldset>

      <p class="pane-note">Both are optional — you can add them after the room is live.</p>
    </div>
  {/if}

  {#if errorMessage}
    <p class="error-banner">{errorMessage}</p>
  {/if}

  {#if isReadOnly}
    <p class="warn-banner">
      This signer is read-only. Switch to a writable signer to create a room.
    </p>
  {/if}

  <footer class="wizard-foot">
    {#if step > 1}
      <button type="button" class="btn btn-ghost" onclick={goBack} disabled={publishing}>
        Back
      </button>
    {:else}
      <span></span>
    {/if}

    {#if step < 3}
      <button
        type="button"
        class="btn btn-primary"
        onclick={goNext}
        disabled={step === 1 && !step1Complete}
      >
        Continue
      </button>
    {:else}
      <button
        type="button"
        class="btn btn-primary"
        onclick={publishRoom}
        disabled={publishing || isReadOnly || !step1Complete}
      >
        {publishing ? 'Creating…' : 'Create the room'}
      </button>
    {/if}
  </footer>
</section>

<style>
  .wizard {
    display: grid;
    gap: 2rem;
    max-width: 38rem;
    margin: 0 auto;
    padding: 3rem 0 4rem;
  }

  .wizard-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .wizard-step {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .wizard-dots {
    display: inline-flex;
    gap: 0.4rem;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-base-300);
  }

  .dot.active {
    background: var(--accent);
  }

  .dot.done {
    background: var(--text-strong);
  }

  .pane {
    display: grid;
    gap: 1.5rem;
    min-height: 18rem;
  }

  .pane-head {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 5vw, 2.8rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .pane-note {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.55;
  }

  .big-field {
    display: grid;
    gap: 0.4rem;
  }

  .big-label {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .big-field input {
    width: 100%;
    padding: 0.9rem 0;
    border: 0;
    border-bottom: 1px solid var(--color-base-300);
    background: transparent;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.5rem, 3.5vw, 2rem);
    line-height: 1.15;
    letter-spacing: -0.02em;
    outline: none;
    transition: border-color 120ms ease;
  }

  .big-field input::placeholder {
    color: var(--muted);
    opacity: 0.55;
  }

  .big-field input:focus {
    border-color: var(--accent);
  }

  .slug-row {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: baseline;
    gap: 0;
    padding: 0.65rem 0;
    border-bottom: 1px dotted var(--color-base-300);
  }

  .slug-prefix {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 0.82rem;
  }

  .slug-input {
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-strong);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    outline: none;
  }

  .slug-hint,
  .slug-warn {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.5;
  }

  .slug-hint {
    color: var(--muted);
  }

  .slug-warn {
    color: var(--pale-red-text);
  }

  .presets {
    display: grid;
    gap: 0.75rem;
  }

  .preset {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1rem;
    align-items: start;
    padding: 1.1rem 1.25rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1rem;
    background: var(--surface);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .preset:hover {
    border-color: var(--text-strong);
  }

  .preset.active {
    border-color: var(--accent);
    background: rgba(255, 103, 25, 0.04);
  }

  .preset input {
    margin: 0.35rem 0 0;
  }

  .preset-body {
    display: grid;
    gap: 0.25rem;
  }

  .preset-body strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 500;
    letter-spacing: -0.01em;
  }

  .preset-body p {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.55;
  }

  .field-input {
    width: 100%;
    padding: 0.625rem 0.75rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.75rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.9rem;
    font-family: inherit;
    outline: none;
    transition: border-color 120ms ease;
    resize: vertical;
  }

  .field-input::placeholder {
    color: var(--muted);
  }

  .field-input:focus {
    border-color: var(--accent);
  }

  .fieldset {
    display: grid;
    gap: 0.5rem;
    border: none;
    padding: 0;
    margin: 0;
  }

  .fieldset-legend {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .error-banner,
  .warn-banner {
    margin: 0;
    padding: 0.85rem 1rem;
    border-radius: 0.9rem;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .error-banner {
    background: var(--pale-red);
    color: var(--pale-red-text);
  }

  .warn-banner {
    background: var(--pale-yellow);
    color: var(--pale-yellow-text);
  }

  .wizard-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--color-base-300);
  }
</style>
