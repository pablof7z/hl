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

<div class="flex justify-center px-5 py-16">
  <div class="grid w-full max-w-[28rem] gap-7">
    <div class="grid gap-[0.35rem]">
      <h1 class="m-0 font-serif text-[1.65rem] font-bold text-base-content leading-[1.2] tracking-[-0.02em]">
        Set up your profile
      </h1>
      <p class="m-0 text-base-content/50 text-[0.95rem]">Let readers know who you are.</p>
    </div>

    <!-- Avatar -->
    <div class="flex justify-center">
      <button
        type="button"
        class="flex flex-col items-center gap-[0.6rem] bg-transparent border-none cursor-pointer p-0 group"
        onclick={handleAvatarClick}
        aria-label="Choose avatar image"
      >
        {#if avatarDisplayUrl}
          <img
            src={avatarDisplayUrl}
            alt="Avatar preview"
            class="w-24 h-24 rounded-full object-cover border-2 border-base-300"
          />
        {:else}
          <div
            class="w-24 h-24 rounded-full bg-base-200 border-2 border-dashed border-base-300 flex items-center justify-center transition-colors duration-[140ms] group-hover:border-primary"
            aria-hidden="true"
          >
            <svg viewBox="0 0 24 24" class="w-10 h-10 text-base-content/50 fill-none stroke-current stroke-[1.5] [stroke-linecap:round] [stroke-linejoin:round]">
              <path d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z" />
            </svg>
          </div>
        {/if}
        <span class="text-[0.82rem] text-primary font-semibold">
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
    <div class="grid gap-2">
      <fieldset class="fieldset">
        <legend class="fieldset-legend">
          Display name <span class="text-primary" aria-hidden="true">*</span>
        </legend>
        <input
          id="setup-display-name"
          type="text"
          bind:value={displayName}
          placeholder="Your name"
          class="w-full px-3 py-[0.625rem] border border-base-300 rounded-[0.75rem] bg-base-200 text-base-content text-sm font-[inherit] outline-none transition-colors duration-[120ms] ease placeholder:text-base-content/50 focus:border-primary resize-y"
          maxlength="64"
          autocomplete="name"
        />
      </fieldset>

      <fieldset class="fieldset">
        <legend class="fieldset-legend">Bio <span class="opacity-60 font-normal">(optional)</span></legend>
        <textarea
          id="setup-bio"
          bind:value={bio}
          placeholder="Tell people a bit about yourself…"
          class="w-full px-3 py-[0.625rem] border border-base-300 rounded-[0.75rem] bg-base-200 text-base-content text-sm font-[inherit] outline-none transition-colors duration-[120ms] ease placeholder:text-base-content/50 focus:border-primary resize-y"
          rows="3"
          maxlength="500"
        ></textarea>
      </fieldset>
    </div>

    <!-- Errors -->
    {#if uploadError}
      <p class="m-0 px-[0.85rem] py-[0.65rem] rounded-box bg-error/10 text-error text-[0.88rem]" role="alert">{uploadError}</p>
    {/if}
    {#if saveError}
      <p class="m-0 px-[0.85rem] py-[0.65rem] rounded-box bg-error/10 text-error text-[0.88rem]" role="alert">{saveError}</p>
    {/if}

    <!-- Actions -->
    <div class="flex flex-col gap-[0.6rem]">
      <button
        type="button"
        class="btn btn-primary w-full"
        onclick={handleSave}
        disabled={!canSave}
        aria-busy={saving}
      >
        {saving ? 'Saving…' : 'Save profile'}
      </button>
      <button type="button" class="btn btn-ghost w-full" onclick={handleSkip}>
        Skip for now
      </button>
    </div>
  </div>
</div>
