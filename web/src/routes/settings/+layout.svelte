<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import type { LayoutProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import LoginDialog from '$lib/features/auth/LoginDialog.svelte';

  let { children }: LayoutProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  const sessionsReady = $derived(ndk.$sessions !== undefined);
  const pathname = $derived(page.url.pathname);

  const activeSection = $derived(
    pathname.startsWith('/settings/media') ? 'media' : 'network'
  );

  $effect(() => {
    if (!browser) return;
    if (sessionsReady && !currentUser) {
      void goto('/discover');
    }
  });
</script>

<svelte:head>
  <title>Settings — Highlighter</title>
</svelte:head>

{#if currentUser}
  <div class="settings-shell">
    <!-- Sidebar (desktop) / Tab bar (mobile) -->
    <nav class="settings-nav" aria-label="Settings sections">
      <a
        href="/settings/network"
        class="settings-nav-item"
        class:active={activeSection === 'network'}
        aria-current={activeSection === 'network' ? 'page' : undefined}
      >
        <svg class="size-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M12 21a9.004 9.004 0 0 0 8.716-6.747M12 21a9.004 9.004 0 0 1-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 0 1 7.843 4.582M12 3a8.997 8.997 0 0 0-7.843 4.582m15.686 0A11.953 11.953 0 0 1 12 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0 1 21 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0 1 12 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 0 1 3 12c0-1.605.42-3.113 1.157-4.418" />
        </svg>
        Network
      </a>
      <a
        href="/settings/media"
        class="settings-nav-item"
        class:active={activeSection === 'media'}
        aria-current={activeSection === 'media' ? 'page' : undefined}
      >
        <svg class="size-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="m2.25 15.75 5.159-5.159a2.25 2.25 0 0 1 3.182 0l5.159 5.159m-1.5-1.5 1.409-1.409a2.25 2.25 0 0 1 3.182 0l2.909 2.909m-18 3.75h16.5a1.5 1.5 0 0 0 1.5-1.5V6a1.5 1.5 0 0 0-1.5-1.5H3.75A1.5 1.5 0 0 0 2.25 6v12a1.5 1.5 0 0 0 1.5 1.5Zm10.5-11.25h.008v.008h-.008V8.25Zm.375 0a.375.375 0 1 1-.75 0 .375.375 0 0 1 .75 0Z" />
        </svg>
        Media
      </a>
      <a
        href="/profile/edit"
        class="settings-nav-item"
      >
        <svg class="size-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10" />
        </svg>
        Profile
      </a>
    </nav>

    <div class="settings-content">
      {@render children?.()}
    </div>
  </div>
{:else if !sessionsReady}
  <!-- Sessions restoring -->
{:else}
  <div class="flex justify-center items-start px-5 py-20">
    <div class="flex flex-col items-center gap-4 max-w-sm text-center">
      <div class="size-12 text-primary">
        <svg class="size-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z"
          />
        </svg>
      </div>
      <h2 class="m-0 text-xl font-bold tracking-tight text-base-content">Sign in to continue</h2>
      <p class="m-0 text-sm leading-relaxed text-base-content/60">
        Settings require a Nostr identity.
      </p>
      <LoginDialog />
    </div>
  </div>
{/if}

<style>
  .settings-shell {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 2rem;
    max-width: 900px;
    margin: 2rem auto;
    padding: 0 1rem;
    align-items: start;
  }

  @media (max-width: 640px) {
    .settings-shell {
      grid-template-columns: 1fr;
      gap: 0;
      margin: 0;
      padding: 0;
    }
  }

  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    position: sticky;
    top: 4.5rem;
  }

  @media (max-width: 640px) {
    .settings-nav {
      flex-direction: row;
      position: static;
      border-bottom: 1px solid var(--color-base-200, oklch(93% 0.01 250));
      padding: 0 1rem;
      gap: 0;
    }
  }

  .settings-nav-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-radius: 0.5rem;
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--color-base-content);
    opacity: 0.65;
    text-decoration: none;
    transition: background-color 0.15s, opacity 0.15s;
  }

  .settings-nav-item:hover {
    background-color: var(--color-base-200, oklch(93% 0.01 250));
    opacity: 1;
  }

  .settings-nav-item.active {
    background-color: var(--color-base-200, oklch(93% 0.01 250));
    opacity: 1;
    color: var(--color-base-content);
  }

  @media (max-width: 640px) {
    .settings-nav-item {
      border-radius: 0;
      padding: 0.625rem 1rem;
      border-bottom: 2px solid transparent;
      margin-bottom: -1px;
    }

    .settings-nav-item.active {
      background-color: transparent;
      border-bottom-color: var(--color-primary, oklch(50% 0.2 260));
      color: var(--color-primary, oklch(50% 0.2 260));
      opacity: 1;
    }
  }

  .settings-content {
    min-width: 0;
    padding: 0 0 3rem;
  }

  @media (max-width: 640px) {
    .settings-content {
      padding: 1.5rem 1rem 3rem;
    }
  }
</style>
