<script lang="ts">
  import { setContext, onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import '$lib/features/room/styles/tokens.css';
  import TopNav from '$lib/features/room/components/TopNav.svelte';
  import Footer from '$lib/features/room/components/Footer.svelte';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { NDK_CONTEXT_KEY } from '$lib/ndk/utils/ndk';
  import type { LayoutData } from './$types';

  let {
    data,
    children
  }: {
    data: LayoutData;
    children: Snippet;
  } = $props();

  setContext(NDK_CONTEXT_KEY, ndk);

  onMount(() => {
    void ensureClientNdk().catch((error) => {
      console.error('Failed to connect client NDK', error);
    });
  });
</script>

{#if data.isRoomEnabled}
  <div class="room-shell">
    <TopNav activeLink="rooms" />
    <div class="page">
      {@render children()}
    </div>
    <Footer />
  </div>
{:else}
  <div class="coming-soon">
    <p>Room view coming soon</p>
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

  .room-shell {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .page {
    max-width: var(--container-max);
    margin: 0 auto;
    padding: 0 var(--container-px);
    width: 100%;
    flex: 1;
  }

  .coming-soon {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 40vh;
    padding: 2rem;
  }

  .coming-soon p {
    font-family: var(--font-sans);
    font-size: 1rem;
    color: var(--ink-fade);
  }
</style>
