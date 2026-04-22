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
  <section class="me-shell">
    <header class="vault-header">
      <div class="vault-identity">
        {#if profileImage}
          <img class="vault-avatar" src={profileImage} alt="" />
        {:else}
          <div class="vault-avatar vault-avatar-fallback" aria-hidden="true">
            {displayLabel.charAt(0).toUpperCase() || '#'}
          </div>
        {/if}

        <div class="vault-copy">
          <h1>{displayLabel}</h1>
          <div class="vault-meta">
            {#if profileNip05}
              <span>{profileNip05}</span>
            {/if}
            {#if currentUser}
              <span>{shortPubkey(currentUser.pubkey)}</span>
            {/if}
          </div>
          <p class="vault-bio">
            {profileBio || 'Highlights, saved items, and rooms in one place.'}
          </p>
        </div>
      </div>

      <div class="vault-stats">
        <div class="stat-card">
          <span>Highlights</span>
          <strong>{authoredHighlightFeed.events.length}</strong>
        </div>
        <div class="stat-card">
          <span>Rooms</span>
          <strong>{roomCount}</strong>
        </div>
      </div>
    </header>

    <nav class="me-tabs">
      {#each meTabs as tab (tab.href)}
        <a
          href={tab.href}
          class="me-tab"
          class:active={isActive(tab.href)}
          aria-current={isActive(tab.href) ? 'page' : undefined}
        >
          {tab.label}
        </a>
      {/each}
    </nav>

    <div class="me-slot">
      {@render children?.()}
    </div>
  </section>
{/if}

<style>
  .me-shell {
    display: grid;
    gap: 1.4rem;
    padding: 1rem 0 3rem;
  }

  .vault-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 1.25rem;
    padding: 1.25rem 0;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 38%),
      transparent;
  }

  .vault-identity,
  .vault-meta,
  .vault-stats,
  .me-tabs {
    display: flex;
    gap: 0.7rem;
    flex-wrap: wrap;
  }

  .vault-identity {
    align-items: start;
  }

  .vault-avatar {
    width: 4.75rem;
    height: 4.75rem;
    border-radius: 1.2rem;
    object-fit: cover;
    background: var(--surface-soft);
  }

  .vault-avatar-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 1.5rem;
    font-weight: 800;
  }

  .vault-copy {
    display: grid;
    gap: 0.45rem;
    max-width: 42rem;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2.1rem, 5vw, 3rem);
    line-height: 1;
    letter-spacing: -0.04em;
  }

  .vault-meta span {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.7rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 700;
  }

  .stat-card span {
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .vault-bio {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .vault-stats {
    align-content: start;
    justify-content: end;
  }

  .stat-card {
    display: grid;
    gap: 0.2rem;
    min-width: 5rem;
    text-align: center;
  }

  .stat-card strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.9rem;
    line-height: 1;
  }

  .me-tabs {
    gap: 0.5rem;
  }

  .me-tab {
    padding: 0.45rem 1rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-base-300);
    background: var(--surface);
    color: var(--text);
    font-size: 0.88rem;
    font-weight: 500;
    text-decoration: none;
    transition: border-color 140ms, color 140ms;
  }

  .me-tab:hover,
  .me-tab.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .me-slot {
    display: grid;
  }

  @media (max-width: 820px) {
    .vault-header {
      grid-template-columns: 1fr;
    }

    .vault-stats {
      justify-content: start;
    }
  }
</style>
