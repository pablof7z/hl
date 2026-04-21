<script lang="ts">
  interface Props {
    pending: boolean;
    secretKey?: string;
    onLogin?: () => void | Promise<void>;
  }

  let { pending, secretKey = $bindable(''), onLogin }: Props = $props();
</script>

<div class="auth-form">
  <label class="auth-form-field">
    <span class="auth-form-label">Secret key</span>
    <textarea
      class="auth-form-textarea"
      bind:value={secretKey}
      placeholder="Paste your secret key (nsec… or hex)"
    ></textarea>
  </label>
  <button
    class="auth-form-btn-primary"
    type="button"
    onclick={() => void onLogin?.()}
    disabled={pending || !secretKey.trim()}
  >
    {pending ? 'Signing in...' : 'Continue with key'}
  </button>
</div>

<style>
  .auth-form {
    display: grid;
    gap: 0.85rem;
  }

  .auth-form-field {
    display: grid;
    gap: 0.4rem;
  }

  .auth-form-label {
    color: var(--muted);
    font-size: 0.82rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .auth-form-textarea {
    width: 100%;
    min-height: 5.5rem;
    padding: 0.6rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-soft);
    color: var(--text-strong);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    line-height: 1.5;
    resize: vertical;
    transition: border-color 160ms ease;
    box-sizing: border-box;
  }

  .auth-form-textarea:focus {
    outline: none;
    border-color: var(--accent);
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
