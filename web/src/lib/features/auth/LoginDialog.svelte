<script lang="ts">
  import { goto } from '$app/navigation';
  import { NDKNip07Signer, NDKNip46Signer, NDKPrivateKeySigner } from '@nostr-dev-kit/ndk';
  import { onDestroy } from 'svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Tabs from '$lib/components/ui/tabs';
  import { ndk } from '$lib/ndk/client';
  import ExtensionLoginForm from './ExtensionLoginForm.svelte';
  import PrivateKeyLoginForm from './PrivateKeyLoginForm.svelte';
  import RemoteLoginForm from './RemoteLoginForm.svelte';
  import {
    hasNostrExtension,
    prepareRemoteSignerPairing,
    stopNostrConnectSigner,
    type LoginMode
  } from './auth';

  let { showTrigger = true, open = $bindable(false) }: {
    showTrigger?: boolean;
    open?: boolean;
  } = $props();

  let mode = $state<LoginMode>('extension');
  let pending = $state(false);
  let preparingRemoteSigner = $state(false);
  let connectingBunker = $state(false);
  let privateKey = $state('');
  let bunkerUri = $state('');
  let qrCodeDataUrl = $state('');
  let nostrConnectUri = $state('');
  let nostrConnectSigner: NDKNip46Signer | null = $state(null);
  let error = $state('');

  const extensionAvailable = $derived(hasNostrExtension());
  const remoteSignerReady = $derived(Boolean(qrCodeDataUrl && nostrConnectUri));

  function clearRemoteSigner() {
    bunkerUri = '';
    qrCodeDataUrl = '';
    nostrConnectUri = '';
    connectingBunker = false;
    stopNostrConnectSigner(nostrConnectSigner);
    nostrConnectSigner = null;
  }

  function resetDialogState() {
    error = '';
    pending = false;
    privateKey = '';
    mode = 'extension';
    preparingRemoteSigner = false;
    clearRemoteSigner();
  }

  $effect(() => {
    if (!open) {
      resetDialogState();
    }
  });

  $effect(() => {
    if (mode !== 'remote') {
      preparingRemoteSigner = false;
      clearRemoteSigner();
    }
  });

  function finishLogin() {
    open = false;
  }

  function startJoin() {
    void goto('/onboarding');
  }

  async function loginWithExtension() {
    if (!ndk.$sessions || pending || !extensionAvailable) return;

    try {
      pending = true;
      error = '';
      await ndk.$sessions.login(new NDKNip07Signer());
      finishLogin();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "Couldn't log in with the extension.";
    } finally {
      pending = false;
    }
  }

  async function loginWithPrivateKey() {
    if (!ndk.$sessions || pending || !privateKey.trim()) return;

    try {
      pending = true;
      error = '';
      await ndk.$sessions.login(new NDKPrivateKeySigner(privateKey.trim()));
      finishLogin();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "Couldn't log in with that key.";
    } finally {
      pending = false;
    }
  }

  async function startRemoteSigner() {
    if (!ndk.$sessions || preparingRemoteSigner || connectingBunker) return;

    try {
      error = '';
      clearRemoteSigner();
      preparingRemoteSigner = true;

      const pairing = await prepareRemoteSignerPairing(ndk);
      const activeSigner = pairing.signer;
      nostrConnectSigner = activeSigner;
      nostrConnectUri = pairing.nostrConnectUri;
      qrCodeDataUrl = pairing.qrCodeDataUrl;

      void ndk.$sessions
        .login(activeSigner)
        .then(async () => {
          if (nostrConnectSigner !== activeSigner) return;
          finishLogin();
        })
        .catch((caught) => {
          if (nostrConnectSigner !== activeSigner) return;
          error = caught instanceof Error ? caught.message : "Couldn't finish connecting to that app.";
        });
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "Couldn't start pairing with another app.";
      clearRemoteSigner();
    } finally {
      preparingRemoteSigner = false;
    }
  }

  async function loginWithBunker() {
    if (!ndk.$sessions || connectingBunker || !bunkerUri.trim().startsWith('bunker://')) return;

    try {
      error = '';
      connectingBunker = true;
      stopNostrConnectSigner(nostrConnectSigner);
      nostrConnectSigner = null;
      await ndk.$sessions.login(new NDKNip46Signer(ndk, bunkerUri.trim()));
      finishLogin();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : "Couldn't use that connection link.";
    } finally {
      connectingBunker = false;
    }
  }

  onDestroy(() => {
    stopNostrConnectSigner(nostrConnectSigner);
  });
</script>

<div class="auth-panel">
  <Dialog.Root bind:open>
    {#if showTrigger}
      <div class="auth-guest-actions">
        <button class="auth-btn-outline" type="button" onclick={startJoin}>Join</button>
        <Dialog.Trigger class="auth-btn-primary">Log in</Dialog.Trigger>
      </div>
    {/if}

    <Dialog.Content class="auth-dialog">
      <div class="auth-dialog-chrome">
        <div class="auth-dialog-handle" aria-hidden="true"></div>

        <Dialog.Header class="auth-dialog-header">
          <Dialog.Title>Log in</Dialog.Title>
          <Dialog.Description>
            Choose how you want to log in. Your session stays on this device.
          </Dialog.Description>
        </Dialog.Header>

        <Dialog.Close class="btn btn-circle btn-ghost btn-sm" aria-label="Close login">
          <svg class="size-4" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
          </svg>
        </Dialog.Close>
      </div>

      <div class="auth-dialog-body">
        <Tabs.Root bind:value={mode}>
          <Tabs.List>
            <Tabs.Trigger value="extension">Extension</Tabs.Trigger>
            <Tabs.Trigger value="private-key">Secret key</Tabs.Trigger>
            <Tabs.Trigger value="remote">Another app</Tabs.Trigger>
          </Tabs.List>

          <Tabs.Content value="extension">
            <ExtensionLoginForm
              hasExtension={extensionAvailable}
              {pending}
              onLogin={loginWithExtension}
            />
          </Tabs.Content>

          <Tabs.Content value="private-key">
            <PrivateKeyLoginForm
              bind:secretKey={privateKey}
              {pending}
              onLogin={loginWithPrivateKey}
            />
          </Tabs.Content>

          <Tabs.Content value="remote">
            <RemoteLoginForm
              bind:bunkerUri
              {connectingBunker}
              {nostrConnectUri}
              {preparingRemoteSigner}
              {qrCodeDataUrl}
              {remoteSignerReady}
              onLoginWithBunker={loginWithBunker}
              onStartRemoteSigner={startRemoteSigner}
            />
          </Tabs.Content>
        </Tabs.Root>

        {#if error}
          <p class="auth-error">{error}</p>
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Root>
</div>
