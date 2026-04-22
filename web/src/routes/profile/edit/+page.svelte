<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    NDKBlossomList,
    NDKEvent,
    NDKKind,
    type NDKUserProfile,
    type NostrEvent
  } from '@nostr-dev-kit/ndk';
  import { onDestroy } from 'svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { cleanText, displayName, profileIdentifier } from '$lib/ndk/format';
  import {
    DEFAULT_BLOSSOM_SERVER,
    blossomServerFromEvent,
    mergeBlossomServers,
    parseBlossomServer
  } from '$lib/onboarding';
  import ProfilePreview from '$lib/features/profile/ProfilePreview.svelte';

  // ── profile fields ─────────────────────────────────────────────
  let activePubkey = $state<string | null>(null);
  let resolvedProfile: NDKUserProfile | undefined = $state();
  let name = $state('');
  let display = $state('');
  let about = $state('');
  let website = $state('');
  let nip05 = $state('');
  let lud16 = $state('');
  let avatarUrl = $state('');
  let bannerUrl = $state('');
  let avatarFile: File | null = $state(null);
  let bannerFile: File | null = $state(null);
  let avatarPreviewUrl = $state('');
  let bannerPreviewUrl = $state('');
  let profileTouched = $state(false);
  let profileLoading = $state(false);
  let blossomServer = $state(DEFAULT_BLOSSOM_SERVER);
  let blossomTouched = $state(false);
  let saving = $state(false);
  let uploadingAvatar = $state(false);
  let uploadingBanner = $state(false);
  let saveError = $state('');
  let uploadError = $state('');
  let avatarFileInput: HTMLInputElement | null = $state(null);
  let bannerFileInput: HTMLInputElement | null = $state(null);

  // ── NIP-F1 fields ──────────────────────────────────────────────
  let backgroundColor = $state('');
  let foregroundColor = $state('');
  let backgroundMusic = $state('');
  let priorityKinds = $state<number[]>([]);
  let customFields = $state<Array<{ key: string; value: string }>>([]);
  let nipF1Touched = $state(false);
  let nipF1Loading = $state(false);

  // ── preview collapsed state ────────────────────────────────────
  let previewOpen = $state(false);

  // ── derived ────────────────────────────────────────────────────
  const currentUser = $derived(ndk.$currentUser);
  const blossomEvent = $derived(ndk.$sessions?.getSessionEvent(NDKKind.BlossomList));
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const avatarDisplayUrl = $derived(avatarPreviewUrl || avatarUrl || '');
  const bannerDisplayUrl = $derived(bannerPreviewUrl || bannerUrl || '');
  const previewName = $derived(cleanText(display) || cleanText(name) || '');
  const canPublish = $derived(!isReadOnly && !saving && !uploadingAvatar && !uploadingBanner);

  const PRIORITY_KIND_OPTIONS = [
    { kind: 1, label: 'Notes' },
    { kind: 30023, label: 'Articles' },
    { kind: 30024, label: 'Drafts' }
  ];

  // ── helpers ────────────────────────────────────────────────────
  function clearMessages() {
    saveError = '';
    uploadError = '';
  }

  function clearAvatarPreview() {
    if (avatarPreviewUrl) {
      URL.revokeObjectURL(avatarPreviewUrl);
      avatarPreviewUrl = '';
    }
  }

  function clearBannerPreview() {
    if (bannerPreviewUrl) {
      URL.revokeObjectURL(bannerPreviewUrl);
      bannerPreviewUrl = '';
    }
  }

  function seedProfile(profile: NDKUserProfile | undefined) {
    resolvedProfile = profile ? { ...profile } : undefined;
    if (!profileTouched) {
      name = cleanText(profile?.name);
      display = cleanText(profile?.displayName);
      about = cleanText(profile?.about || profile?.bio);
      website = cleanText(profile?.website);
      nip05 = cleanText(profile?.nip05);
      lud16 = cleanText(profile?.lud16);
      avatarUrl = cleanText(profile?.picture || profile?.image);
      bannerUrl = cleanText(profile?.banner);
    }
  }

  function seedNipF1(event: NDKEvent) {
    if (nipF1Touched) return;
    const tags = event.tags;

    backgroundColor = tags.find((t) => t[0] === 'background-color')?.[1] ?? '';
    foregroundColor = tags.find((t) => t[0] === 'foreground-color')?.[1] ?? '';
    backgroundMusic = tags.find((t) => t[0] === 'background-music')?.[1] ?? '';

    const kindsTag = tags.find((t) => t[0] === 'priority_kinds')?.[1];
    priorityKinds = kindsTag
      ? kindsTag.split(',').map(Number).filter((n) => !isNaN(n))
      : [];

    customFields = tags
      .filter((t) => t[0] === 'custom' && t[1] && t[2])
      .map((t) => ({ key: t[1], value: t[2] }));
  }

  function handleAvatarClick() {
    avatarFileInput?.click();
  }

  function handleBannerClick() {
    bannerFileInput?.click();
  }

  function handleAvatarSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0] ?? null;
    clearMessages();
    clearAvatarPreview();
    avatarFile = file;
    if (file) avatarPreviewUrl = URL.createObjectURL(file);
    profileTouched = true;
  }

  function handleBannerSelection(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0] ?? null;
    clearMessages();
    clearBannerPreview();
    bannerFile = file;
    if (file) bannerPreviewUrl = URL.createObjectURL(file);
    profileTouched = true;
  }

  async function uploadFile(file: File): Promise<string | null> {
    const hasCustomValue = Boolean(cleanText(blossomServer));
    const parsedServer = parseBlossomServer(blossomServer) ?? (hasCustomValue ? null : DEFAULT_BLOSSOM_SERVER);
    if (!parsedServer) {
      uploadError = 'Enter a valid storage server URL.';
      return null;
    }

    await ensureClientNdk();
    const { NDKBlossom } = await import('@nostr-dev-kit/blossom');
    const blossom = new NDKBlossom(ndk);
    const descriptor = await blossom.upload(file, { server: parsedServer });
    const uploadedUrl = descriptor.url;
    if (!uploadedUrl) throw new Error("The storage server didn't return a file URL.");

    blossomTouched = true;
    blossomServer = parsedServer;
    return uploadedUrl;
  }

  async function uploadAvatarFile(): Promise<string | null> {
    if (!avatarFile) return avatarUrl || null;
    try {
      clearMessages();
      uploadingAvatar = true;
      const url = await uploadFile(avatarFile);
      if (url) {
        avatarUrl = url;
        avatarFile = null;
        clearAvatarPreview();
        if (avatarFileInput) avatarFileInput.value = '';
      }
      return url;
    } catch (caught) {
      uploadError = caught instanceof Error ? caught.message : "Couldn't upload the avatar.";
      return null;
    } finally {
      uploadingAvatar = false;
    }
  }

  async function uploadBannerFile(): Promise<string | null> {
    if (!bannerFile) return bannerUrl || null;
    try {
      clearMessages();
      uploadingBanner = true;
      const url = await uploadFile(bannerFile);
      if (url) {
        bannerUrl = url;
        bannerFile = null;
        clearBannerPreview();
        if (bannerFileInput) bannerFileInput.value = '';
      }
      return url;
    } catch (caught) {
      uploadError = caught instanceof Error ? caught.message : "Couldn't upload the banner.";
      return null;
    } finally {
      uploadingBanner = false;
    }
  }

  function togglePriorityKind(kind: number) {
    nipF1Touched = true;
    priorityKinds = priorityKinds.includes(kind)
      ? priorityKinds.filter((k) => k !== kind)
      : [...priorityKinds, kind];
  }

  function addCustomField() {
    nipF1Touched = true;
    customFields = [...customFields, { key: '', value: '' }];
  }

  function removeCustomField(index: number) {
    nipF1Touched = true;
    customFields = customFields.filter((_, i) => i !== index);
  }

  // ── publish ────────────────────────────────────────────────────
  async function publish() {
    if (!currentUser || !canPublish) return;

    let nextAvatar = cleanText(avatarUrl);
    let nextBanner = cleanText(bannerUrl);

    if (avatarFile) {
      const url = await uploadAvatarFile();
      if (!url) { saveError = uploadError || 'Avatar upload failed.'; return; }
      nextAvatar = cleanText(url);
    }

    if (bannerFile) {
      const url = await uploadBannerFile();
      if (!url) { saveError = uploadError || 'Banner upload failed.'; return; }
      nextBanner = cleanText(url);
    }

    try {
      clearMessages();
      saving = true;
      await ensureClientNdk();

      // ── kind 0: profile ──────────────────────────────────────
      const previousProfile = currentUser.profile ? { ...currentUser.profile } : undefined;
      const nextProfile: NDKUserProfile = { ...(currentUser.profile ?? {}) };
      nextProfile.name = cleanText(name) || undefined;
      nextProfile.displayName = cleanText(display) || undefined;
      nextProfile.about = cleanText(about) || undefined;
      nextProfile.bio = cleanText(about) || undefined;
      nextProfile.website = cleanText(website) || undefined;
      nextProfile.picture = nextAvatar || undefined;
      nextProfile.image = nextAvatar || undefined;
      nextProfile.banner = nextBanner || undefined;
      nextProfile.nip05 = cleanText(nip05) || undefined;
      nextProfile.lud16 = cleanText(lud16) || undefined;

      currentUser.profile = nextProfile;
      try {
        await currentUser.publish();
      } catch (caught) {
        currentUser.profile = previousProfile;
        throw caught;
      }

      // ── blossom list ─────────────────────────────────────────
      if (blossomTouched) {
        const parsedServer = parseBlossomServer(blossomServer);
        if (parsedServer) {
          const session = ndk.$sessions?.current;
          const nextBlossom =
            blossomEvent instanceof NDKBlossomList
              ? blossomEvent
              : blossomEvent
                ? NDKBlossomList.from(blossomEvent as NDKEvent)
                : new NDKBlossomList(ndk);
          nextBlossom.servers = mergeBlossomServers(parsedServer, nextBlossom.servers);
          nextBlossom.default = parsedServer;
          await nextBlossom.publish();
          session?.events.set(NDKKind.BlossomList, nextBlossom);
        }
      }

      // ── kind 19999: NIP-F1 ───────────────────────────────────
      const nipF1Tags: string[][] = [];
      if (cleanText(backgroundColor)) nipF1Tags.push(['background-color', cleanText(backgroundColor)]);
      if (cleanText(foregroundColor)) nipF1Tags.push(['foreground-color', cleanText(foregroundColor)]);
      if (cleanText(backgroundMusic)) nipF1Tags.push(['background-music', cleanText(backgroundMusic)]);
      if (priorityKinds.length > 0) nipF1Tags.push(['priority_kinds', priorityKinds.join(',')]);
      for (const field of customFields) {
        const k = cleanText(field.key);
        const v = cleanText(field.value);
        if (k && v) nipF1Tags.push(['custom', k, v]);
      }

      if (nipF1Tags.length > 0) {
        const nipF1Event = new NDKEvent(ndk, {
          kind: 19999,
          content: '',
          tags: nipF1Tags
        } as NostrEvent);
        await nipF1Event.publish();
      }

      await goto(`/profile/${profileIdentifier(nextProfile, currentUser.npub)}`);
    } catch (caught) {
      saveError = caught instanceof Error ? caught.message : "Couldn't publish your profile.";
    } finally {
      saving = false;
    }
  }

  // ── effects ────────────────────────────────────────────────────
  $effect(() => {
    const pubkey = currentUser?.pubkey ?? null;
    if (activePubkey === pubkey) return;
    activePubkey = pubkey;
    profileTouched = false;
    nipF1Touched = false;
    blossomTouched = false;
    profileLoading = false;
    clearMessages();
    if (currentUser?.profile) seedProfile(currentUser.profile);
  });

  $effect(() => {
    if (!profileTouched) seedProfile(currentUser?.profile ?? resolvedProfile);
  });

  $effect(() => {
    if (!blossomTouched) {
      blossomServer = blossomServerFromEvent(blossomEvent as NDKEvent | null | undefined);
    }
  });

  $effect(() => {
    if (!currentUser?.pubkey || currentUser.profile || profileLoading) return;

    const targetPubkey = currentUser.pubkey;
    profileLoading = true;

    void currentUser.fetchProfile()
      .then((profile) => {
        if (currentUser?.pubkey !== targetPubkey) return;
        resolvedProfile = profile ?? currentUser.profile ?? undefined;
        if (!profileTouched) seedProfile(profile ?? currentUser.profile ?? undefined);
      })
      .catch(() => {
        if (currentUser?.pubkey !== targetPubkey) return;
        resolvedProfile = currentUser.profile ?? undefined;
      })
      .finally(() => {
        if (currentUser?.pubkey === targetPubkey) profileLoading = false;
      });
  });

  // ── fetch NIP-F1 event ─────────────────────────────────────────
  $effect(() => {
    if (!currentUser?.pubkey || nipF1Loading) return;

    const targetPubkey = currentUser.pubkey;
    nipF1Loading = true;

    void ndk.fetchEvent({ kinds: [19999 as NDKKind], authors: [targetPubkey] })
      .then((event) => {
        if (!event || currentUser?.pubkey !== targetPubkey) return;
        seedNipF1(event);
      })
      .catch(() => {})
      .finally(() => {
        if (currentUser?.pubkey === targetPubkey) nipF1Loading = false;
      });
  });

  onDestroy(() => {
    clearAvatarPreview();
    clearBannerPreview();
  });
</script>

{#if !currentUser}
  <section class="mx-auto max-w-2xl px-4 py-12">
    <p class="m-0 text-base-content/60">Log in to edit your profile.</p>
  </section>
{:else}
  <div class="mx-auto grid max-w-[var(--page-width)] grid-cols-[1fr_360px] gap-10 px-0 pb-24 pt-10 max-md:grid-cols-1 max-md:pb-16 max-md:pt-8">
    <div class="grid min-w-0 gap-8">
      <h1 class="m-0 text-[clamp(1.6rem,4vw,2.2rem)] font-bold leading-tight tracking-tight text-base-content">Edit profile</h1>

      <!-- Section: Identity -->
      <div class="grid gap-4">
        <div class="border-b border-base-300 pb-2 text-xs font-semibold uppercase tracking-wider text-base-content/60">Identity</div>

        <div
          class="group relative aspect-[3/1] w-full cursor-pointer overflow-hidden rounded-lg border-2 border-dashed border-base-300 bg-base-200 transition-colors hover:border-base-content/70"
          role="button"
          tabindex="0"
          onclick={handleBannerClick}
          onkeydown={(e) => e.key === 'Enter' && handleBannerClick()}
        >
          {#if bannerDisplayUrl}
            <img src={bannerDisplayUrl} alt="Banner" class="size-full object-cover" />
          {:else}
            <div class="flex h-full flex-col items-center justify-center gap-1.5 text-sm text-base-content/60">
              <svg class="size-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <circle cx="8.5" cy="8.5" r="1.5" />
                <path d="m21 15-5-5L5 21" />
              </svg>
              <span>Upload banner</span>
            </div>
          {/if}
          <input
            bind:this={bannerFileInput}
            type="file"
            accept="image/*"
            onchange={handleBannerSelection}
            class="pointer-events-none absolute size-0 opacity-0"
            tabindex="-1"
          />
        </div>

        <div class="-mt-6 flex items-center gap-4 pl-4 max-md:-mt-4">
          <button
            class="group relative size-28 cursor-pointer overflow-hidden rounded-full border-2 border-dashed border-base-300 bg-base-200 transition-colors hover:border-base-content/70"
            type="button"
            onclick={handleAvatarClick}
            aria-label="Upload avatar"
          >
            {#if avatarDisplayUrl}
              <img src={avatarDisplayUrl} alt="Your avatar" class="block size-full object-cover" />
            {:else}
              <div class="flex h-full flex-col items-center justify-center gap-1.5 text-xs font-medium text-base-content/60">
                <svg class="size-7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                  <circle cx="12" cy="8" r="4" />
                  <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7" />
                </svg>
              </div>
            {/if}
            <div
              class="absolute inset-0 flex items-center justify-center bg-black/45 text-white opacity-0 transition-opacity group-hover:opacity-100"
              aria-hidden="true"
            >
              <svg class="size-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12" />
              </svg>
            </div>
          </button>
          <input
            bind:this={avatarFileInput}
            type="file"
            accept="image/*"
            onchange={handleAvatarSelection}
            class="pointer-events-none absolute size-0 opacity-0"
            tabindex="-1"
          />

          {#if avatarDisplayUrl}
            <button
              class="link link-hover cursor-pointer border-0 bg-transparent p-0 text-xs text-base-content/60"
              type="button"
              onclick={() => {
                avatarUrl = '';
                avatarFile = null;
                clearAvatarPreview();
                if (avatarFileInput) avatarFileInput.value = '';
                profileTouched = true;
              }}
            >Remove avatar</button>
          {/if}
          {#if bannerDisplayUrl}
            <button
              class="link link-hover cursor-pointer border-0 bg-transparent p-0 text-xs text-base-content/60"
              type="button"
              onclick={() => {
                bannerUrl = '';
                bannerFile = null;
                clearBannerPreview();
                if (bannerFileInput) bannerFileInput.value = '';
                profileTouched = true;
              }}
            >Remove banner</button>
          {/if}
        </div>

        {#if uploadError}
          <p class="m-0 text-sm text-error">{uploadError}</p>
        {/if}

        <div class="grid gap-4">
          <div class="grid grid-cols-2 gap-4 max-md:grid-cols-1">
            <fieldset class="fieldset">
              <legend class="fieldset-legend">Display name</legend>
              <input class="input w-full" bind:value={display} oninput={() => { profileTouched = true; }} placeholder="Your full name" />
            </fieldset>
            <fieldset class="fieldset">
              <legend class="fieldset-legend">Username</legend>
              <input class="input w-full" bind:value={name} oninput={() => { profileTouched = true; }} placeholder="username" />
            </fieldset>
          </div>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Bio</legend>
            <textarea class="textarea w-full" bind:value={about} oninput={() => { profileTouched = true; }} placeholder="Tell people about yourself" rows="3"></textarea>
          </fieldset>
        </div>
      </div>

      <!-- Section: Links & Verification -->
      <div class="grid gap-4">
        <div class="border-b border-base-300 pb-2 text-xs font-semibold uppercase tracking-wider text-base-content/60">Links & Verification</div>

        <div class="grid gap-4">
          <fieldset class="fieldset">
            <legend class="fieldset-legend">NIP-05</legend>
            <input class="input w-full" bind:value={nip05} oninput={() => { profileTouched = true; }} placeholder="you@example.com" />
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Lightning address</legend>
            <input class="input w-full" bind:value={lud16} oninput={() => { profileTouched = true; }} placeholder="you@wallet.com" />
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Website</legend>
            <input class="input w-full" bind:value={website} oninput={() => { profileTouched = true; }} placeholder="https://yoursite.com" type="url" />
          </fieldset>
        </div>
      </div>

      <!-- Section: Appearance (NIP-F1) -->
      <div class="grid gap-4">
        <div class="border-b border-base-300 pb-2 text-xs font-semibold uppercase tracking-wider text-base-content/60">Appearance</div>

        <div class="grid gap-4">
          <div class="grid grid-cols-2 gap-4 max-md:grid-cols-1">
            <fieldset class="fieldset">
              <legend class="fieldset-legend">Background color</legend>
              <div class="grid grid-cols-[2.5rem_1fr] items-center gap-2">
                <input
                  class="size-10 cursor-pointer rounded-md border border-base-300 bg-base-100 p-0.5"
                  type="color"
                  value={backgroundColor || '#ffffff'}
                  oninput={(e) => { backgroundColor = (e.currentTarget as HTMLInputElement).value; nipF1Touched = true; }}
                />
                <input
                  class="input w-full font-mono"
                  type="text"
                  bind:value={backgroundColor}
                  oninput={() => { nipF1Touched = true; }}
                  placeholder="#ffffff"
                />
              </div>
            </fieldset>
            <fieldset class="fieldset">
              <legend class="fieldset-legend">Text color</legend>
              <div class="grid grid-cols-[2.5rem_1fr] items-center gap-2">
                <input
                  class="size-10 cursor-pointer rounded-md border border-base-300 bg-base-100 p-0.5"
                  type="color"
                  value={foregroundColor || '#000000'}
                  oninput={(e) => { foregroundColor = (e.currentTarget as HTMLInputElement).value; nipF1Touched = true; }}
                />
                <input
                  class="input w-full font-mono"
                  type="text"
                  bind:value={foregroundColor}
                  oninput={() => { nipF1Touched = true; }}
                  placeholder="#000000"
                />
              </div>
            </fieldset>
          </div>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Background music URL</legend>
            <input class="input w-full" bind:value={backgroundMusic} oninput={() => { nipF1Touched = true; }} placeholder="https://example.com/ambient.mp3" type="url" />
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Priority kinds</legend>
            <div class="flex flex-wrap gap-4">
              {#each PRIORITY_KIND_OPTIONS as opt (opt.kind)}
                <label class="label cursor-pointer gap-2">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-sm"
                    checked={priorityKinds.includes(opt.kind)}
                    onchange={() => togglePriorityKind(opt.kind)}
                  />
                  <span class="label-text">{opt.label} <span class="text-base-content/60">(kind:{opt.kind})</span></span>
                </label>
              {/each}
            </div>
          </fieldset>
        </div>
      </div>

      <!-- Section: Custom Fields (NIP-F1) -->
      <div class="grid gap-4">
        <div class="border-b border-base-300 pb-2 text-xs font-semibold uppercase tracking-wider text-base-content/60">Custom Fields</div>

        <div class="grid gap-2.5">
          {#each customFields as field, index (index)}
            <div class="grid grid-cols-[8rem_1fr_auto] items-center gap-2 max-md:grid-cols-[1fr_1fr_auto]">
              <input
                type="text"
                placeholder="Key"
                bind:value={field.key}
                oninput={() => { nipF1Touched = true; }}
                class="input w-full"
              />
              <input
                type="text"
                placeholder="Value"
                bind:value={field.value}
                oninput={() => { nipF1Touched = true; }}
                class="input w-full"
              />
              <button
                type="button"
                class="btn btn-square btn-ghost btn-sm text-base-content/60 hover:text-error"
                onclick={() => removeCustomField(index)}
                aria-label="Remove field"
              >
                <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M18 6 6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
          {/each}
          <button type="button" class="btn btn-outline btn-sm w-fit" onclick={addCustomField}>
            Add field
          </button>
        </div>
      </div>

      <!-- Sticky footer -->
      <div class="sticky bottom-0 z-10 grid gap-2 border-t border-base-300 bg-base-100/90 py-4 backdrop-blur-md">
        {#if saveError}
          <p class="m-0 text-sm text-error">{saveError}</p>
        {/if}
        <div class="flex items-center gap-3">
          <button class="btn btn-primary" type="button" disabled={!canPublish} onclick={() => void publish()}>
            {saving ? 'Saving…' : uploadingAvatar || uploadingBanner ? 'Uploading…' : 'Save profile'}
          </button>
          <a href="/profile/{profileIdentifier(currentUser.profile, currentUser.npub)}" class="btn">
            View profile
          </a>
        </div>
      </div>
    </div>

    <!-- Live Preview -->
    <div class="relative">
      <button
        class="btn btn-outline btn-sm hidden w-full max-md:flex"
        type="button"
        onclick={() => { previewOpen = !previewOpen; }}
      >
        {previewOpen ? 'Hide preview' : 'Show preview'}
      </button>
      <div
        class="sticky top-20 max-md:static"
        class:max-md:hidden={!previewOpen}
      >
        <ProfilePreview
          name={previewName}
          bio={cleanText(about)}
          avatarUrl={avatarDisplayUrl}
          bannerUrl={bannerDisplayUrl}
          nip05={cleanText(nip05)}
          website={cleanText(website)}
          backgroundColor={cleanText(backgroundColor)}
          foregroundColor={cleanText(foregroundColor)}
          customFields={customFields.filter((f) => f.key && f.value)}
        />
      </div>
    </div>
  </div>
{/if}
