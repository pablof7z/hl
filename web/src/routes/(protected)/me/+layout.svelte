<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import type { LayoutProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { avatarUrl, displayName, displayNip05, shortPubkey } from '$lib/ndk/format';
  import { groupIdFromEvent } from '$lib/ndk/groups';
  import { HIGHLIGHTER_HIGHLIGHT_KIND, resolveUserHighlightRelayUrls } from '$lib/ndk/highlights';
  import { profileHasBasics } from '$lib/onboarding';

  let { children }: LayoutProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  const pathname = $derived(page.url.pathname);
  const hideVaultChrome = $derived(pathname === '/me/setup');
  const displayLabel = $derived(
    currentUser ? displayName(currentUser.profile, shortPubkey(currentUser.pubkey)) : 'My Vault'
  );
  const profileImage = $derived(currentUser ? avatarUrl(currentUser.profile) : undefined);
  const profileNip05 = $derived(currentUser ? displayNip05(currentUser.profile) : '');
  const profileBio = $derived(currentUser?.profile?.about?.trim() || currentUser?.profile?.bio?.trim() || '');
  let highlightRelayUrls = $state<string[]>(DEFAULT_RELAYS);

  $effect(() => {
    if (!browser || !currentUser || pathname === '/me/setup') {
      return;
    }

    if (ndk.$sessions !== undefined && !profileHasBasics(currentUser.profile)) {
      void goto('/me/setup');
    }
  });

  $effect(() => {
    if (!browser || !currentUser || hideVaultChrome) {
      highlightRelayUrls = DEFAULT_RELAYS;
      return;
    }

    let cancelled = false;

    void resolveUserHighlightRelayUrls(ndk, currentUser.pubkey).then((relayUrls) => {
      if (!cancelled) {
        highlightRelayUrls = relayUrls;
      }
    });

    return () => {
      cancelled = true;
    };
  });

  const authoredHighlightFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser || hideVaultChrome) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_KIND], authors: [currentUser.pubkey], limit: 256 }],
      relayUrls: highlightRelayUrls,
      closeOnEose: true
    };
  });

  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser || hideVaultChrome) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const roomCount = $derived.by(() => {
    const ids = new Set<string>();

    for (const event of membershipFeed.events) {
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }

    return ids.size;
  });

  const meTabs = [
    { href: '/me/highlights', label: 'Highlights' },
    { href: '/me/rooms', label: 'Rooms' },
    { href: '/me/for-later', label: 'For Later' },
    { href: '/me/recommended', label: 'Recommended' }
  ];

  function isActive(href: string): boolean {
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>

{#if hideVaultChrome}
  {@render children?.()}
{:else}
  <section class="grid gap-6 py-4 pb-12">
    <header
      class="grid grid-cols-[minmax(0,1fr)_auto] gap-5 py-5 max-[820px]:grid-cols-1"
      style="background: radial-gradient(circle at top left, rgba(255,103,25,0.08), transparent 38%), transparent;"
    >
      <div class="flex flex-wrap items-start gap-3">
        {#if profileImage}
          <img
            class="h-[4.75rem] w-[4.75rem] rounded-[1.2rem] object-cover bg-base-200"
            src={profileImage}
            alt=""
          />
        {:else}
          <div
            class="grid h-[4.75rem] w-[4.75rem] place-items-center rounded-[1.2rem] bg-base-200 text-primary text-2xl font-extrabold"
            aria-hidden="true"
          >
            {displayLabel.charAt(0).toUpperCase() || '#'}
          </div>
        {/if}

        <div class="grid gap-[0.45rem] max-w-[42rem]">
          <h1 class="m-0 font-serif text-base-content leading-none tracking-[-0.04em]" style="font-size: clamp(2.1rem, 5vw, 3rem);">
            {displayLabel}
          </h1>
          <div class="flex flex-wrap gap-[0.7rem]">
            {#if profileNip05}
              <span class="inline-flex items-center min-h-[1.9rem] px-3 rounded-full bg-base-200 text-base-content/50 text-xs font-bold">
                {profileNip05}
              </span>
            {/if}
            {#if currentUser}
              <span class="inline-flex items-center min-h-[1.9rem] px-3 rounded-full bg-base-200 text-base-content/50 text-xs font-bold">
                {shortPubkey(currentUser.pubkey)}
              </span>
            {/if}
          </div>
          <p class="m-0 text-base-content/50 leading-relaxed">
            {profileBio || 'Highlights, saved items, and rooms in one place.'}
          </p>
        </div>
      </div>

      <div class="flex flex-wrap gap-[0.7rem] content-start justify-end max-[820px]:justify-start">
        <div class="grid gap-1 min-w-[5rem] text-center">
          <span class="text-base-content/50 text-[0.72rem] font-semibold uppercase tracking-[0.06em]">Highlights</span>
          <strong class="font-serif text-[1.9rem] leading-none text-base-content">{authoredHighlightFeed.events.length}</strong>
        </div>
        <div class="grid gap-1 min-w-[5rem] text-center">
          <span class="text-base-content/50 text-[0.72rem] font-semibold uppercase tracking-[0.06em]">Rooms</span>
          <strong class="font-serif text-[1.9rem] leading-none text-base-content">{roomCount}</strong>
        </div>
      </div>
    </header>

    <nav class="flex flex-wrap gap-2">
      {#each meTabs as tab (tab.href)}
        <a
          href={tab.href}
          class="px-4 py-[0.45rem] rounded-box border border-base-300 bg-base-100 text-base-content text-[0.88rem] font-medium no-underline transition-colors duration-[140ms] hover:border-primary hover:text-primary"
          class:border-primary={isActive(tab.href)}
          class:text-primary={isActive(tab.href)}
          aria-current={isActive(tab.href) ? 'page' : undefined}
        >
          {tab.label}
        </a>
      {/each}
    </nav>

    <div class="grid">
      {@render children?.()}
    </div>
  </section>
{/if}
