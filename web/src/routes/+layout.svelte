<script lang="ts">
  import { page } from '$app/state';
  import { onMount, setContext } from 'svelte';
  import type { LayoutProps } from './$types';
  import '../app.css';
  import AuthPanel from '$lib/features/auth/AuthPanel.svelte';
  import HeaderSearch from '$lib/components/HeaderSearch.svelte';
  import SiteNavigation from '$lib/components/SiteNavigation.svelte';
  import SeoHead from '$lib/components/SeoHead.svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import type { SeoMetadata } from '$lib/seo';
  import { NDK_CONTEXT_KEY } from '$lib/ndk/utils/ndk';

  let { children }: LayoutProps = $props();
  const seo = $derived((page.data as { seo?: SeoMetadata }).seo);
  const inRoomShell = $derived(page.url.pathname.startsWith('/room/'));
  const signedIn = $derived(Boolean(ndk.$currentUser));
  // On the marketing landing, the page renders its own top nav.
  // Suppress the global app navbar for guests at "/"; signed-in users still get it.
  const hideGlobalShell = $derived(inRoomShell || (page.url.pathname === '/' && !signedIn));

  setContext(NDK_CONTEXT_KEY, ndk);

  onMount(() => {
    void ensureClientNdk().catch((error) => {
      console.error('Failed to connect client NDK', error);
    });
  });
</script>

{#if seo}
  <SeoHead {seo} />
{/if}

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link
    href="https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,300;0,9..144,400;0,9..144,500;0,9..144,600;1,9..144,400;1,9..144,500&family=Inter:wght@300;400;500;600;700&family=Caveat:wght@400;500;600;700&family=JetBrains+Mono:wght@400&display=swap"
    rel="stylesheet"
  />
</svelte:head>

{#if hideGlobalShell}
  {@render children?.()}
{:else}
  <header class="app-navbar-shell">
    <div class="shell navbar min-h-0 px-0 py-2 gap-4">
      <div class="navbar-start gap-4">
        <a class="brand" href="/">
          <span class="brand-name">Highlighter</span>
          <span class="brand-dot" aria-hidden="true"></span>
        </a>
        <SiteNavigation />
      </div>
      <div class="navbar-end gap-2">
        <HeaderSearch />
        <AuthPanel />
      </div>
    </div>
  </header>

  <main class="shell page">
    {@render children?.()}
  </main>

  <footer class="shell footer">
    <div class="footer-grid">
      <div class="footer-logo">
        <span class="footer-logo-mark"></span>
        Highlighter
      </div>
      <div class="footer-links">
        <a href="/about">About</a>
        <a href="/discover">Discover</a>
      </div>
      <span class="footer-note">
        Built on Nostr. Your circles, your data, always.
        <a href="/changelog" class="commit-hash">{__COMMIT_HASH__}</a>
      </span>
    </div>
  </footer>
{/if}

<style>
  .app-navbar-shell {
    position: sticky;
    top: 0;
    z-index: 20;
    background: rgba(248, 245, 240, 0.92);
    backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-light);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    text-decoration: none;
  }

  .brand-name {
    font-family: var(--font-serif);
    font-size: 1.2rem;
    font-weight: 700;
    color: var(--text-strong);
    letter-spacing: -0.02em;
  }

  .brand-dot {
    display: inline-block;
    width: 0.42rem;
    height: 0.42rem;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
    margin-bottom: 0.08rem;
  }

  .footer-logo {
    display: flex;
    align-items: center;
    font-weight: 600;
    font-size: 0.95rem;
    color: var(--text-strong);
  }

  .footer-logo-mark {
    display: inline-block;
    width: 1rem;
    height: 1rem;
    background: var(--accent);
    border-radius: 3px;
    margin-right: 0.5rem;
  }

  .footer-links {
    display: flex;
    gap: 1.5rem;
  }

  .footer-links a {
    font-size: 0.88rem;
    color: var(--muted);
    transition: color 0.15s;
  }

  .footer-links a:hover {
    color: var(--text-strong);
  }

  .footer-note {
    font-size: 0.82rem;
    color: var(--muted);
  }

  .commit-hash {
    font-family: monospace;
    font-size: 0.75rem;
    color: var(--muted);
    opacity: 0.6;
    text-decoration: none;
    margin-left: 0.5rem;
  }

  .commit-hash:hover {
    opacity: 1;
    color: var(--accent);
  }

</style>
