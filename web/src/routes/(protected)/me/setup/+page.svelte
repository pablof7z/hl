<script lang="ts">
  import { goto } from '$app/navigation';
  import { NDKEvent, NDKKind, type NDKUserProfile } from '@nostr-dev-kit/ndk';
  import { onDestroy } from 'svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { cleanText } from '$lib/ndk/format';
  import { DEFAULT_BLOSSOM_SERVER, parseBlossomServer } from '$lib/onboarding';

  // ── state ──────────────────────────────────────────────────────
  let displayName = $state('');
  let bio = $state('');
  let avatarUrl = $state('');
  let avatarFile: File | null = $state(null);
  let avatarPreviewUrl = $state('');
  let avatarFileInput: HTMLInputElement | null = $state(null);

  let saving = $state(false);
  let uploadingAvatar = $state(false);
  let saveError = $state('');
  let uploadError = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const canSave = $derived(!isReadOnly && !saving && !uploadingAvatar && cleanText(displayName).length > 0);
  const avatarDisplayUrl = $derived(avatarPreviewUrl || avatarUrl);

  // ── seed from existing profile ─────────────────────────────────
  let seeded = false;
  $effect(() => {
    if (currentUser?.profile && !seeded) {
      seeded = true;
      const p = currentUser.profile;
      displayName = cleanText(p.displayName || p.name);
      bio = cleanText(p.about || p.bio);
      avatarUrl = cleanText(p.picture || p.image);
    }
  });

  // ── avatar handling ────────────────────────────────────────────
  function handleAvatarClick() {
    avatarFileInput?.click();
  }

  function handleAvatarSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0] ?? null;
    uploadError = '';
    if (avatarPreviewUrl) URL.revokeObjectURL(avatarPreviewUrl);
    avatarFile = file;
    avatarPreviewUrl = file ? URL.createObjectURL(file) : '';
  }

  async function uploadAvatarFile(): Promise<string | null> {
    if (!avatarFile) return avatarUrl || null;
    const server = parseBlossomServer(DEFAULT_BLOSSOM_SERVER) ?? DEFAULT_BLOSSOM_SERVER;
    try {
      uploadError = '';
      uploadingAvatar = true;
      await ensureClientNdk();
      const { NDKBlossom } = await import('@nostr-dev-kit/blossom');
      const blossom = new NDKBlossom(ndk);
      const descriptor = await blossom.upload(avatarFile, { server });
      const url = descriptor.url;
      if (!url) throw new Error("The storage server didn't return a file URL.");
      avatarUrl = url;
      avatarFile = null;
      if (avatarPreviewUrl) {
        URL.revokeObjectURL(avatarPreviewUrl);
        avatarPreviewUrl = '';
      }
      if (avatarFileInput) avatarFileInput.value = '';
      return url;
    } catch (caught) {
      uploadError = caught instanceof Error ? caught.message : "Couldn't upload the avatar.";
      return null;
    } finally {
      uploadingAvatar = false;
    }
  }

  // ── save ───────────────────────────────────────────────────────
  async function handleSave() {
    if (!canSave || !currentUser) return;

    saveError = '';
    saving = true;

    try {
      // Upload avatar if a file was selected
      const finalAvatarUrl = await uploadAvatarFile();
      if (avatarFile && !finalAvatarUrl) {
        // Upload failed, error already set
        return;
      }

      // Build kind:0 profile
      const profile: NDKUserProfile = {
        ...(currentUser.profile ?? {}),
        displayName: cleanText(displayName),
        name: cleanText(displayName),
        about: cleanText(bio) || undefined,
        picture: finalAvatarUrl || undefined
      };

      const event = new NDKEvent(ndk);
      event.kind = NDKKind.Metadata;
      event.content = JSON.stringify(profile);
      await event.publish();

      await goto('/me');
    } catch (caught) {
      saveError = caught instanceof Error ? caught.message : "Couldn't save your profile.";
    } finally {
      saving = false;
    }
  }

  function handleSkip() {
    void goto('/me');
  }

  onDestroy(() => {
    if (avatarPreviewUrl) URL.revokeObjectURL(avatarPreviewUrl);
  });
</script>

<svelte:head>
  <title>Set up your profile — Highlighter</title>
</svelte:head>

<div class="setup-page">
  <div class="setup-card">
    <div class="setup-header">
      <h1 class="setup-title">Set up your profile</h1>
      <p class="setup-subtitle">Let the community know who you are.</p>
    </div>

    <!-- Avatar -->
    <div class="avatar-section">
      <button
        type="button"
        class="avatar-pick"
        onclick={handleAvatarClick}
        aria-label="Choose avatar image"
      >
        {#if avatarDisplayUrl}
          <img src={avatarDisplayUrl} alt="Avatar preview" class="avatar-img" />
        {:else}
          <div class="avatar-placeholder" aria-hidden="true">
            <svg viewBox="0 0 24 24" class="avatar-placeholder-icon">
              <path d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z" />
            </svg>
          </div>
        {/if}
        <span class="avatar-pick-label">
          {uploadingAvatar ? 'Uploading…' : 'Choose photo'}
        </span>
      </button>
      <input
        bind:this={avatarFileInput}
        type="file"
        accept="image/*"
        class="sr-only"
        onchange={handleAvatarSelection}
        aria-label="Avatar file input"
      />
    </div>

    <!-- Fields -->
    <div class="setup-fields">
      <label class="field-label" for="setup-display-name">
        Display name <span class="field-required" aria-hidden="true">*</span>
      </label>
      <input
        id="setup-display-name"
        type="text"
        bind:value={displayName}
        placeholder="Your name"
        class="field-input"
        maxlength="64"
        autocomplete="name"
      />

      <label class="field-label" for="setup-bio">Bio <span class="field-optional">(optional)</span></label>
      <textarea
        id="setup-bio"
        bind:value={bio}
        placeholder="Tell people a bit about yourself…"
        class="field-textarea"
        rows="3"
        maxlength="500"
      ></textarea>
    </div>

    <!-- Errors -->
    {#if uploadError}
      <p class="setup-error" role="alert">{uploadError}</p>
    {/if}
    {#if saveError}
      <p class="setup-error" role="alert">{saveError}</p>
    {/if}

    <!-- Actions -->
    <div class="setup-actions">
      <button
        type="button"
        class="btn-primary"
        onclick={handleSave}
        disabled={!canSave}
        aria-busy={saving}
      >
        {saving ? 'Saving…' : 'Save profile'}
      </button>
      <button type="button" class="btn-ghost" onclick={handleSkip}>
        Skip for now
      </button>
    </div>
  </div>
</div>

<style>
  .setup-page {
    display: flex;
    justify-content: center;
    padding: 4rem 1.25rem;
  }

  .setup-card {
    width: 100%;
    max-width: 28rem;
    display: grid;
    gap: 1.75rem;
  }

  .setup-header {
    display: grid;
    gap: 0.35rem;
  }

  .setup-title {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 1.65rem;
    font-weight: 700;
    color: var(--text-strong);
    line-height: 1.2;
    letter-spacing: -0.02em;
  }

  .setup-subtitle {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
  }

  /* Avatar */
  .avatar-section {
    display: flex;
    justify-content: center;
  }

  .avatar-pick {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
  }

  .avatar-img {
    width: 6rem;
    height: 6rem;
    border-radius: 50%;
    object-fit: cover;
    border: 2px solid var(--border);
  }

  .avatar-placeholder {
    width: 6rem;
    height: 6rem;
    border-radius: 50%;
    background: var(--surface-soft);
    border: 2px dashed var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: border-color 140ms ease;
  }

  .avatar-pick:hover .avatar-placeholder {
    border-color: var(--accent);
  }

  .avatar-placeholder-icon {
    width: 2.5rem;
    height: 2.5rem;
    color: var(--muted);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .avatar-pick-label {
    font-size: 0.82rem;
    color: var(--accent);
    font-weight: 600;
  }

  /* Fields */
  .setup-fields {
    display: grid;
    gap: 0.5rem;
  }

  .field-label {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-strong);
    margin-top: 0.5rem;
  }

  .field-required {
    color: var(--accent);
  }

  .field-optional {
    color: var(--muted);
    font-weight: 400;
  }

  .field-input,
  .field-textarea {
    width: 100%;
    padding: 0.65rem 0.85rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    color: var(--text);
    font-size: 0.95rem;
    line-height: 1.5;
    transition: border-color 140ms ease;
    box-sizing: border-box;
  }

  .field-input:focus,
  .field-textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(255, 103, 25, 0.12);
  }

  .field-textarea {
    resize: vertical;
    min-height: 5rem;
  }

  /* Errors */
  .setup-error {
    margin: 0;
    padding: 0.65rem 0.85rem;
    border-radius: var(--radius-md);
    background: var(--pale-red);
    color: var(--pale-red-text);
    font-size: 0.88rem;
  }

  /* Actions */
  .setup-actions {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .btn-primary {
    width: 100%;
    padding: 0.75rem 1.25rem;
    border: none;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #ffffff;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 140ms ease, opacity 140ms ease;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .btn-ghost {
    width: 100%;
    padding: 0.65rem;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 0.88rem;
    cursor: pointer;
    transition: color 120ms ease;
  }

  .btn-ghost:hover {
    color: var(--text);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }
</style>
