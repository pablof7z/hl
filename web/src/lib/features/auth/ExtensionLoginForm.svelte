<script lang="ts">
  interface Props {
    hasExtension: boolean;
    pending: boolean;
    onLogin?: () => void | Promise<void>;
  }

  let { hasExtension, pending, onLogin }: Props = $props();
</script>

<div class="auth-form">
  <p class="auth-form-hint">Use a browser extension you already trust.</p>
  <button
    class="auth-form-btn-primary"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !hasExtension}
  >
    {pending ? 'Connecting...' : 'Continue with extension'}
  </button>
  {#if !hasExtension}
    <p class="auth-form-hint">No extension detected. Install a Nostr extension like Alby or nos2x first.</p>
  {/if}
</div>

<style>
  .auth-form {
    display: grid;
    gap: 0.85rem;
  }

  .auth-form-hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.5;
  }

  .auth-form-btn-primary {
    width: 100%;
    padding: 0.72rem 1.25rem;
    border: none;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #ffffff;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 140ms ease, opacity 140ms ease;
  }

  .auth-form-btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .auth-form-btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
