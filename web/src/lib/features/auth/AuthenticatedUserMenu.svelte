<script lang="ts">
  import type { NDKUser, NDKUserProfile } from '@nostr-dev-kit/ndk';
  import { goto } from '$app/navigation';
  import * as Avatar from '$lib/components/ui/avatar';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import {
    authUserAvatar,
    authUserInitials,
    authUserLabel,
    authUserMeta
  } from './auth';

  interface Props {
    user: NDKUser;
    profile: NDKUserProfile | undefined;
    profileHref: string;
    shouldFinishOnboarding: boolean;
    onLogout: () => Promise<void>;
  }

  let { user, profile, profileHref, shouldFinishOnboarding, onLogout }: Props = $props();

  const userLabel = $derived(authUserLabel(profile));
  const userMeta = $derived(authUserMeta(profile, user.npub || user.pubkey || ''));
  const userAvatar = $derived(authUserAvatar(profile));
  const userInitials = $derived(authUserInitials(profile));

  const fallbackClass =
    'bg-gradient-to-br from-[#fbf3db] to-[#edf3ec] text-base-content text-xs uppercase tracking-wider';

  function navigateToCapture() {
    void goto('/capture');
  }

  function navigateToProfile() {
    void goto(profileHref);
  }

  function navigateToEditProfile() {
    void goto('/profile/edit');
  }

  function navigateToSettings() {
    void goto('/settings/network');
  }

  function navigateToSetup() {
    void goto('/me/setup');
  }

  function handleLogout() {
    void onLogout();
  }
</script>

<div class="grid justify-items-end gap-3 max-md:justify-items-end">
  <DropdownMenu.Root>
    <DropdownMenu.Trigger
      class="inline-flex w-full max-w-72 items-center gap-3 rounded-full border border-base-300 bg-transparent px-2 py-2 transition-colors hover:bg-base-200 data-[state=open]:bg-base-200"
      aria-label="Open account menu"
    >
      <Avatar.Root class="size-10 border border-black/10">
        {#if userAvatar}
          <Avatar.Image src={userAvatar} alt={userLabel} />
        {/if}
        <Avatar.Fallback class={fallbackClass}>{userInitials}</Avatar.Fallback>
      </Avatar.Root>

      <span class="grid min-w-0 gap-px text-left">
        <span class="truncate text-sm font-bold leading-tight text-base-content">{userLabel}</span>
        <span class="truncate text-xs leading-tight text-base-content/60 max-sm:hidden">{userMeta}</span>
      </span>

      <svg
        class="ml-auto size-4 shrink-0 text-base-content/60 transition-transform"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.75"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M6.75 9.75 12 15l5.25-5.25" />
      </svg>
    </DropdownMenu.Trigger>

    <DropdownMenu.Content class="w-[min(21rem,calc(100vw-1.5rem))] py-2">
      <div class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3.5 px-3.5 pb-2 pt-1">
        <Avatar.Root class="size-12 border border-black/10">
          {#if userAvatar}
            <Avatar.Image src={userAvatar} alt={userLabel} />
          {/if}
          <Avatar.Fallback class={fallbackClass}>{userInitials}</Avatar.Fallback>
        </Avatar.Root>

        <div class="grid min-w-0 gap-px">
          <span class="truncate text-sm font-bold leading-tight text-base-content">{userLabel}</span>
          <span class="truncate text-xs leading-tight text-base-content/60">{userMeta}</span>
        </div>
      </div>

      <DropdownMenu.Separator />

      <DropdownMenu.Item onSelect={navigateToCapture}>
        <svg
          class="size-4 shrink-0 text-base-content/60"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M17.593 3.322c1.1.128 1.907 1.077 1.907 2.185V21L12 17.25 4.5 21V5.507c0-1.108.806-2.057 1.907-2.185a48.507 48.507 0 0 1 11.186 0Z" />
        </svg>
        <span>Capture</span>
      </DropdownMenu.Item>

      <DropdownMenu.Item onSelect={navigateToProfile}>
        <svg
          class="size-4 shrink-0 text-base-content/60"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z" />
        </svg>
        <span>Profile</span>
      </DropdownMenu.Item>

      <DropdownMenu.Item onSelect={navigateToEditProfile}>
        <svg
          class="size-4 shrink-0 text-base-content/60"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10" />
        </svg>
        <span>Edit profile</span>
      </DropdownMenu.Item>

      <DropdownMenu.Item onSelect={navigateToSettings}>
        <svg
          class="size-4 shrink-0 text-base-content/60"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z" />
          <path d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
        </svg>
        <span>Settings</span>
      </DropdownMenu.Item>

      {#if shouldFinishOnboarding}
        <DropdownMenu.Item onSelect={navigateToSetup}>
          <svg
            class="size-4 shrink-0 text-base-content/60"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
          </svg>
          <span>Set up profile</span>
        </DropdownMenu.Item>
      {/if}

      <DropdownMenu.Item class="text-error" onSelect={handleLogout}>
        <svg
          class="size-4 shrink-0 text-error"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M8.25 9V5.25A2.25 2.25 0 0 1 10.5 3h6a2.25 2.25 0 0 1 2.25 2.25v13.5A2.25 2.25 0 0 1 16.5 21h-6a2.25 2.25 0 0 1-2.25-2.25V15m-3 0-3-3m0 0 3-3m-3 3H15" />
        </svg>
        <span>Sign out</span>
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
</div>
