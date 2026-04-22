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

<div class="grid gap-3.5">
  <p class="m-0 text-sm leading-relaxed text-base-content/60">
    Pair with another app. Show a QR code to approve this session, or paste a connection link.
  </p>

  {#if remoteSignerReady}
    <div class="grid justify-items-center gap-3.5 rounded-2xl border border-base-300 bg-base-200 p-4">
      <a
        class="inline-flex rounded-2xl border border-base-300 bg-white p-3.5 transition-all hover:border-base-content hover:shadow-lg active:scale-[0.98]"
        href={nostrConnectUri}
        title="Open in app"
      >
        <img class="block h-auto w-[min(100%,15rem)] rounded-md" src={qrCodeDataUrl} alt="Connection QR code" />
      </a>
      <div class="badge badge-info justify-self-center">Waiting for approval</div>
      <p class="m-0 text-center text-sm text-base-content/60">
        Open the QR in another app on this device, or scan it from another one.
      </p>
    </div>
  {:else}
    <div class="grid gap-2">
      <button
        class="btn btn-primary w-full"
        type="button"
        onclick={() => void onStartRemoteSigner?.()}
        disabled={preparingRemoteSigner || connectingBunker}
      >
        {preparingRemoteSigner ? 'Preparing QR...' : 'Show QR code'}
      </button>
      <p class="m-0 text-center text-sm leading-relaxed text-base-content/60">
        This starts a one-time pairing request and waits for approval.
      </p>
    </div>
  {/if}

  <div class="flex items-center gap-3 text-xs font-semibold uppercase tracking-wider text-base-content/60">
    <span class="h-px flex-1 bg-base-300"></span>
    <span>or paste a link</span>
    <span class="h-px flex-1 bg-base-300"></span>
  </div>

  <fieldset class="fieldset">
    <legend class="fieldset-legend">Connection link</legend>
    <input
      class="input w-full"
      bind:value={bunkerUri}
      placeholder="bunker://…"
    />
  </fieldset>

  <button
    class="btn btn-primary w-full"
    type="button"
    onclick={() => void onLoginWithBunker?.()}
    disabled={connectingBunker || !bunkerUri.trim().startsWith('bunker://')}
  >
    {connectingBunker ? 'Connecting...' : 'Continue with link'}
  </button>
</div>
