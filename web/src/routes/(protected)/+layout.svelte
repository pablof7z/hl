<script lang="ts">
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import type { LayoutProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import LoginDialog from '$lib/features/auth/LoginDialog.svelte';

  let { children }: LayoutProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  const sessionsReady = $derived(ndk.$sessions !== undefined);

  $effect(() => {
    if (!browser) return;
    if (sessionsReady && !currentUser) {
      void goto('/discover');
    }
  });
</script>

{#if currentUser}
  {@render children?.()}
{:else if !sessionsReady}
  <!-- Sessions restoring — render nothing to avoid flash -->
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
        This page requires a Nostr identity. Log in with a browser extension, remote signer, or secret key.
      </p>
      <LoginDialog />
    </div>
  </div>
{/if}
