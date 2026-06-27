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
  import MiniPlayer from '$lib/features/podcasts/MiniPlayer.svelte';
  import {
    attachAudioElement,
    pause,
    podcastPlayer,
    reportEnded,
    reportLoadedMetadata,
    reportPause,
    reportPlay,
    reportTimeUpdate,
    resume,
    seek
  } from '$lib/features/podcasts/playerStore.svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import type { SeoMetadata } from '$lib/seo';
  import { NDK_CONTEXT_KEY } from '$lib/ndk/utils/ndk';
  import { NDKNip07Signer } from '@nostr-dev-kit/ndk';
  import { browser } from '$app/environment';
  import { getClient } from '$lib/nmp/client.svelte';
  import { hasNostrExtension } from '$lib/features/auth/auth';

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

  let audioEl = $state<HTMLAudioElement | null>(null);
  const playerState = $derived(podcastPlayer.snapshot());

  $effect(() => {
    attachAudioElement(audioEl);
    return () => attachAudioElement(null);
  });

  // Wire navigator.mediaSession (Chrome / Safari) so OS-level controls
  // (lock screen / media keys / AirPods double-tap) drive playback.
  // Best-effort — no-op when the API is missing.
  $effect(() => {
    if (typeof navigator === 'undefined' || !('mediaSession' in navigator)) return;
    const ms = navigator.mediaSession;
    const episode = playerState.episode;
    if (!episode) {
      try {
        ms.metadata = null;
      } catch {
        // ignore
      }
      return;
    }
    try {
      ms.metadata = new MediaMetadata({
        title: episode.title,
        artist: episode.showTitle ?? '',
        artwork: episode.imageUrl ? [{ src: episode.imageUrl, sizes: '512x512' }] : []
      });
      ms.setActionHandler('play', () => resume());
      ms.setActionHandler('pause', () => pause());
      ms.setActionHandler('seekto', (details) => {
        if (typeof details.seekTime === 'number') seek(details.seekTime);
      });
      ms.setActionHandler('seekforward', () => seek(playerState.position + 15));
      ms.setActionHandler('seekbackward', () => seek(Math.max(0, playerState.position - 15)));
    } catch {
      // ignore — older browsers throw on unsupported actions
    }
  });

  onMount(() => {
    void ensureClientNdk().catch((error) => {
      console.error('Failed to connect client NDK', error);
    });
  });

  // #65 S2 — NIP-07 session-restore effect.
  //
  // When the user reloads with an active extension session, re-installs the
  // NIP-07 identity in the wasm bridge so it survives reload. Fires whenever
  // ndk.$currentUser changes (login OR session restore from localStorage).
  //
  // Gate: browser + window.nostr present + active signer is NDKNip07Signer.
  // This ensures we ONLY call setSigner for extension accounts, never for
  // private-key (NDKPrivateKeySigner) or bunker (NDKNip46Signer) sessions —
  // those are NDK-only (upstream #2119/#2068).
  $effect(() => {
    const user = ndk.$currentUser;
    if (!browser || !user || !hasNostrExtension()) return;
    if (!(ndk.signer instanceof NDKNip07Signer)) return;
    // Best-effort, non-blocking. Must not throw or break any existing NDK flow.
    void getClient()
      .setSigner(user.pubkey)
      .catch((err: unknown) => {
        console.warn('[nmp] session-restore setSigner failed (best-effort):', err);
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

<!-- Persistent global podcast player. The hidden <audio> element lives
     in the root layout so navigation never tears it down; the
     MiniPlayer is the visible affordance and shows whenever a podcast
     is loaded. -->
<audio
  bind:this={audioEl}
  preload="metadata"
  ontimeupdate={reportTimeUpdate}
  onloadedmetadata={reportLoadedMetadata}
  onplay={reportPlay}
  onpause={reportPause}
  onended={reportEnded}
></audio>
<MiniPlayer />

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
