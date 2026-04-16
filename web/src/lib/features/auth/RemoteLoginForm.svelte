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

<div class="flex flex-col gap-3">
  <p class="text-base-content/60 text-sm m-0">
    Pair with another app. Show a QR code to approve this session, or paste a connection link.
  </p>

  {#if remoteSignerReady}
    <div class="auth-qr-shell">
      <a class="auth-qr-button" href={nostrConnectUri} title="Open in app">
        <img class="auth-qr-image" src={qrCodeDataUrl} alt="Connection QR code" />
      </a>
      <div class="status-pill status-blue auth-qr-status">Waiting for approval</div>
      <p class="caption auth-qr-caption">
        Open the QR in another app on this device, or scan it from another one.
      </p>
    </div>
  {:else}
    <div class="flex flex-col gap-1.5">
      <button
        class="btn btn-primary w-full"
        type="button"
        onclick={() => void onStartRemoteSigner?.()}
        disabled={preparingRemoteSigner || connectingBunker}
      >
        {preparingRemoteSigner ? 'Preparing QR...' : 'Show QR code'}
      </button>
      <p class="caption auth-qr-caption">
        This starts a one-time pairing request and waits for approval.
      </p>
    </div>
  {/if}

  <div class="divider text-xs uppercase font-semibold tracking-wider">Or paste a link</div>

  <label class="form-control w-full">
    <div class="label">
      <span class="label-text text-base-content/60">Connection link</span>
    </div>
    <input class="input w-full" bind:value={bunkerUri} placeholder="Paste a connection link" />
  </label>
  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLoginWithBunker?.()}
    disabled={connectingBunker || !bunkerUri.trim().startsWith('bunker://')}
  >
    {connectingBunker ? 'Connecting...' : 'Continue with link'}
  </button>
</div>
