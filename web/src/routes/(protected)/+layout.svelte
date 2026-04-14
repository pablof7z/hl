<script lang="ts">
  import { goto } from '$app/navigation';
  import { browser } from '$app/environment';
  import type { LayoutProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import LoginDialog from '$lib/features/auth/LoginDialog.svelte';

  let { children }: LayoutProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  // ndk.$sessions being defined means LocalStorage has been read.
  // Until then, sessions haven't been restored yet — don't flash the gate.
  const sessionsReady = $derived(ndk.$sessions !== undefined);

  $effect(() => {
    if (!browser) return;
    // Once session state is restored and still no user, redirect to discover.
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
  <!-- Sessions ready but no user — show sign-in gate (redirect is also firing) -->
  <div class="auth-gate">
    <div class="auth-gate-card">
      <div class="auth-gate-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M16.5 10.5V6.75a4.5 4.5 0 1 0-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 0 0 2.25-2.25v-6.75a2.25 2.25 0 0 0-2.25-2.25H6.75a2.25 2.25 0 0 0-2.25 2.25v6.75a2.25 2.25 0 0 0 2.25 2.25Z"
          />
        </svg>
      </div>
      <h2 class="auth-gate-title">Sign in to continue</h2>
      <p class="auth-gate-description">
        This page requires a Nostr identity. Log in with a browser extension, remote signer, or secret key.
      </p>
      <LoginDialog />
    </div>
  </div>
{/if}

<style>
  .auth-gate {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 5rem 1.25rem;
  }

  .auth-gate-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    max-width: 380px;
    text-align: center;
  }

  .auth-gate-icon {
    width: 3rem;
    height: 3rem;
    color: var(--accent);
  }

  .auth-gate-icon svg {
    width: 100%;
    height: 100%;
  }

  .auth-gate-title {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 700;
    color: var(--text-strong);
    letter-spacing: -0.01em;
  }

  .auth-gate-description {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.55;
  }
</style>
