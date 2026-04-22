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
  <div class="flex flex-col items-center gap-5 px-6 py-12 text-center">
    <p class="m-0 text-base text-base-content/60">{message}</p>
    <LoginDialog />
  </div>
{/if}
