<script lang="ts">
  interface Props {
    pending: boolean;
    secretKey?: string;
    onLogin?: () => void | Promise<void>;
  }

  let { pending, secretKey = $bindable(''), onLogin }: Props = $props();
</script>

<div class="grid gap-3.5">
  <fieldset class="fieldset">
    <legend class="fieldset-legend">Secret key</legend>
    <textarea
      class="textarea w-full font-mono text-sm"
      rows="3"
      bind:value={secretKey}
      placeholder="Paste your secret key (nsec… or hex)"
    ></textarea>
  </fieldset>
  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !secretKey.trim()}
  >
    {pending ? 'Signing in...' : 'Continue with key'}
  </button>
</div>
