<script lang="ts">
  interface Props {
    pending: boolean;
    secretKey?: string;
    onLogin?: () => void | Promise<void>;
  }

  let { pending, secretKey = $bindable(''), onLogin }: Props = $props();
</script>

<div class="flex flex-col gap-3">
  <label class="form-control w-full">
    <div class="label">
      <span class="label-text text-base-content/60">Secret key</span>
    </div>
    <textarea class="textarea w-full" bind:value={secretKey} placeholder="Paste your secret key"></textarea>
  </label>
  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !secretKey.trim()}
  >
    {pending ? 'Signing in...' : 'Continue with key'}
  </button>
</div>
