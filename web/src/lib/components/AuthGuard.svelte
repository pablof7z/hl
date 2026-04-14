<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import LoginDialog from '$lib/features/auth/LoginDialog.svelte';

  interface Props {
    children?: import('svelte').Snippet;
    message?: string;
  }

  let { children, message = 'Sign in to continue' }: Props = $props();

  const currentUser = $derived(ndk.$currentUser);
</script>

{#if currentUser}
  {@render children?.()}
{:else}
  <div class="auth-guard-prompt">
    <p class="auth-guard-message">{message}</p>
    <LoginDialog />
  </div>
{/if}

<style>
  .auth-guard-prompt {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.25rem;
    padding: 3rem 1.5rem;
    text-align: center;
  }

  .auth-guard-message {
    margin: 0;
    color: var(--muted);
    font-size: 1rem;
  }
</style>
