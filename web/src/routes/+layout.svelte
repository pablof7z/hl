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

<div class="topbar">
  <div class="shell topbar-inner">
    <a class="brand" href="/">
      <span class="brand-name">Highlighter</span>
    </a>

    <SiteNavigation />

    <AuthPanel />
  </div>
</div>

<main class="shell page">
  {@render children?.()}
</main>

<footer class="shell footer">
  <div class="footer-grid">
    <span>Read essays, dispatches, and notes from Nostr.</span>
    <span><a href="/about">About Highlighter</a></span>
  </div>
</footer>
