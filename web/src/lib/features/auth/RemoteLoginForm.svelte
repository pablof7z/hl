<script lang="ts">
  interface Props {
    connectingBunker: boolean;
    bunkerUri?: string;
    nostrConnectUri: string;
    preparingRemoteSigner: boolean;
    qrCodeDataUrl: string;
    remoteSignerReady: boolean;
    onLoginWithBunker?: () => void | Promise<void>;
    onStartRemoteSigner?: () => void | Promise<void>;
  }

  let {
    connectingBunker,
    bunkerUri = $bindable(''),
    nostrConnectUri,
    preparingRemoteSigner,
    qrCodeDataUrl,
    remoteSignerReady,
    onLoginWithBunker,
    onStartRemoteSigner
  }: Props = $props();
</script>

<div class="auth-form">
  <p class="auth-form-hint">
    Pair with another app. Show a QR code to approve this session, or paste a connection link.
  </p>

  {#if remoteSignerReady}
    <div class="auth-qr-shell">
      <a class="auth-qr-button" href={nostrConnectUri} title="Open in app">
        <img class="auth-qr-image" src={qrCodeDataUrl} alt="Connection QR code" />
      </a>
      <div class="badge badge-info auth-qr-status">Waiting for approval</div>
      <p class="caption auth-qr-caption">
        Open the QR in another app on this device, or scan it from another one.
      </p>
    </div>
  {:else}
    <div class="auth-form-group">
      <button
        class="auth-form-btn-primary"
        type="button"
        onclick={() => void onStartRemoteSigner?.()}
        disabled={preparingRemoteSigner || connectingBunker}
      >
        {preparingRemoteSigner ? 'Preparing QR...' : 'Show QR code'}
      </button>
      <p class="auth-form-hint auth-form-hint-center">
        This starts a one-time pairing request and waits for approval.
      </p>
    </div>
  {/if}

  <div class="auth-form-divider"><span>or paste a link</span></div>

  <label class="auth-form-field">
    <span class="auth-form-label">Connection link</span>
    <input
      class="auth-form-input"
      bind:value={bunkerUri}
      placeholder="bunker://…"
    />
  </label>

  <button
    class="auth-form-btn-primary"
    type="button"
    onclick={() => void onLoginWithBunker?.()}
    disabled={connectingBunker || !bunkerUri.trim().startsWith('bunker://')}
  >
    {connectingBunker ? 'Connecting...' : 'Continue with link'}
  </button>
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

  .auth-form-hint-center {
    text-align: center;
  }

  .auth-form-group {
    display: grid;
    gap: 0.5rem;
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

  .auth-form-input {
    width: 100%;
    padding: 0.6rem 0.75rem;
    border: 1px solid var(--color-base-300);
    border-radius: var(--radius-md);
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.92rem;
    transition: border-color 160ms ease;
    box-sizing: border-box;
  }

  .auth-form-input:focus {
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

  .auth-form-divider {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .auth-form-divider::before,
  .auth-form-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--color-base-300);
  }
</style>
