<script lang="ts">
  import { page } from '$app/state';
  import { onMount, setContext } from 'svelte';
  import type { LayoutProps } from './$types';
  import '../app.css';
  import AuthPanel from '$lib/features/auth/AuthPanel.svelte';
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
  <div class="navbar app-navbar shell">
    <div class="navbar-start">
      <a class="brand" href="/">
        <span class="brand-name">Highlighter</span>
        <span class="brand-dot" aria-hidden="true"></span>
      </a>
    </div>

    <div class="navbar-center">
      <SiteNavigation />
    </div>

    <div class="navbar-end">
      <AuthPanel />
    </div>
  </div>
</header>

<main class="shell page">
  {@render children?.()}
</main>

<footer class="shell footer">
  <div class="footer-grid">
    <span>Nostr-native reading communities for artifacts, highlights, and discussion.</span>
    <span><a href="/about">About Highlighter</a></span>
  </div>
</footer>

<style>
  .app-navbar-shell {
    position: sticky;
    top: 0;
    z-index: 20;
    background: var(--canvas);
    border-bottom: 1px solid var(--border);
  }

  .app-navbar {
    gap: 1rem;
    padding: 0.5rem 0;
    min-height: 3.5rem;
    justify-content: space-between;
  }

  .navbar-start,
  .navbar-end {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .navbar-center {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
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
</style>
