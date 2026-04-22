<script lang="ts">
  import { page } from '$app/state';
  import { onMount, setContext } from 'svelte';
  import type { LayoutProps } from './$types';
  import '../app.css';
  import '$lib/features/room/styles/tokens.css';
  import AuthPanel from '$lib/features/auth/AuthPanel.svelte';
  import HeaderSearch from '$lib/components/HeaderSearch.svelte';
  import SeoHead from '$lib/components/SeoHead.svelte';
  import TopNav from '$lib/features/room/components/TopNav.svelte';
  import Footer from '$lib/features/room/components/Footer.svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import type { SeoMetadata } from '$lib/seo';
  import { NDK_CONTEXT_KEY } from '$lib/ndk/utils/ndk';

  let { children }: LayoutProps = $props();
  const seo = $derived((page.data as { seo?: SeoMetadata }).seo);
  const signedIn = $derived(Boolean(ndk.$currentUser));
  const pathname = $derived(page.url.pathname);

  // The landing page at "/" for guests renders a full-bleed marketing surface
  // (marketing TopNav + its own footer) inside the page itself.
  const isGuestLanding = $derived(pathname === '/' && !signedIn);

  const activeLink = $derived(
    pathname.startsWith('/rooms') || pathname.startsWith('/r/') ? 'rooms' :
    pathname.startsWith('/discover') ? 'discover' :
    pathname.startsWith('/vault') || pathname.startsWith('/me/highlights') ? 'vault' :
    undefined
  );

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

{#if isGuestLanding}
  <!-- Landing renders its own chrome inline -->
  {@render children?.()}
{:else}
  <div class="app-shell">
    <TopNav {activeLink}>
      {#snippet right()}
        <HeaderSearch />
        <AuthPanel />
      {/snippet}
    </TopNav>
    <main class="app-main">
      {@render children?.()}
    </main>
    <Footer variant="app" />
  </div>
{/if}

<style>
  :global(html, body) {
    background: var(--bg);
    color: var(--ink);
    font-family: var(--font-sans);
    font-weight: 400;
    font-size: 15px;
    line-height: 1.55;
    margin: 0;
    padding: 0;
    -webkit-font-smoothing: antialiased;
  }

  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }

  .app-shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
  }

  .app-main {
    flex: 1;
    max-width: var(--container-max);
    margin: 0 auto;
    padding: 0 var(--container-px);
    width: 100%;
  }
</style>
