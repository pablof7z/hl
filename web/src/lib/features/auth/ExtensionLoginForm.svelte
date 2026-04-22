<script lang="ts">
  interface Props {
    hasExtension: boolean;
    pending: boolean;
    onLogin?: () => void | Promise<void>;
  }

  let { hasExtension, pending, onLogin }: Props = $props();
</script>

<div class="grid gap-3.5">
  <p class="m-0 text-sm leading-relaxed text-base-content/60">
    Use a browser extension you already trust.
  </p>
  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !hasExtension}
  >
    {pending ? 'Connecting...' : 'Continue with extension'}
  </button>
  {#if !hasExtension}
    <p class="m-0 text-sm leading-relaxed text-base-content/60">
      No extension detected. Install a Nostr extension like Alby or nos2x first.
    </p>
  {/if}
</div>
