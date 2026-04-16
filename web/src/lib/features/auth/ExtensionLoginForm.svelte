<script lang="ts">
  interface Props {
    hasExtension: boolean;
    pending: boolean;
    onLogin?: () => void | Promise<void>;
  }

  let { hasExtension, pending, onLogin }: Props = $props();
</script>

<div class="flex flex-col gap-3">
  <p class="text-base-content/60 text-sm m-0">Use a browser extension you already trust.</p>
  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !hasExtension}
  >
    {pending ? 'Connecting...' : 'Continue with extension'}
  </button>
</div>
