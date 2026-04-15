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

<header class="app-navbar-shell">
  <div class="shell app-header">
    <div class="app-header-top">
      <a class="brand" href="/">
        <span class="brand-name">Highlighter</span>
        <span class="brand-dot" aria-hidden="true"></span>
      </a>

      <div class="header-search-wrap">
        <HeaderSearch />
      </div>

      <div class="header-auth">
        <AuthPanel />
      </div>
    </div>

    <div class="app-header-bottom">
      <SiteNavigation />
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
    <span class="footer-note">Built on Nostr. Your circles, your data, always.</span>
  </div>
</footer>

<style>
  .app-navbar-shell {
    position: sticky;
    top: 0;
    z-index: 20;
    background: rgba(248, 245, 240, 0.92);
    backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-light);
  }

  .app-header {
    display: grid;
    gap: 0.45rem;
    padding: 0.65rem 0 0.6rem;
  }

  .app-header-top {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .header-search-wrap {
    flex: 1;
    min-width: 0;
  }

  .header-auth {
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }

  .app-header-bottom {
    display: flex;
    justify-content: center;
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

  @media (max-width: 900px) {
    .app-header-top {
      gap: 0.65rem;
    }
  }
</style>
