<script lang="ts">
  import { ndk } from '$lib/ndk/client';

  let { pubkey }: { pubkey: string } = $props();

  const currentPubkey = $derived(ndk.$currentUser?.pubkey);
  const isOwnProfile = $derived(Boolean(currentPubkey && currentPubkey === pubkey));
  const isLoggedIn = $derived(Boolean(currentPubkey));

  const isFollowing = $derived(ndk.$follows.has(pubkey));
  const isMuted = $derived(ndk.$mutes?.has(pubkey) ?? false);

  let followHovered = $state(false);
  let pending = $state(false);
  let error = $state<string | null>(null);
  let muteDialogEl = $state<HTMLDialogElement | null>(null);

  async function toggleFollow() {
    if (pending) return;
    pending = true;
    error = null;
    try {
      if (isFollowing) {
        await ndk.$follows.remove(pubkey);
      } else {
        await ndk.$follows.add(pubkey);
      }
    } catch (caught) {
      error = caught instanceof Error ? caught.message : 'Action failed.';
    } finally {
      pending = false;
      followHovered = false;
    }
  }

  function openMuteDialog() {
    error = null;
    muteDialogEl?.showModal();
  }

  async function confirmMute() {
    muteDialogEl?.close();
    if (!ndk.$mutes || pending) return;
    pending = true;
    error = null;
    try {
      await ndk.$mutes.toggle(pubkey);
    } catch (caught) {
      error = caught instanceof Error ? caught.message : 'Action failed.';
    } finally {
      pending = false;
    }
  }
</script>

{#if !isOwnProfile}
  <div class="flex items-center gap-2">
    {#if isLoggedIn}
      <button
        type="button"
        disabled={pending}
        class={followHovered && isFollowing
          ? 'btn btn-ghost btn-error rounded-full'
          : isFollowing
            ? 'btn btn-outline btn-success rounded-full'
            : 'btn btn-primary rounded-full'}
        onmouseenter={() => (followHovered = true)}
        onmouseleave={() => (followHovered = false)}
        onclick={toggleFollow}
      >
        {#if pending}
          <span class="loading loading-spinner loading-xs"></span>
        {:else if followHovered && isFollowing}
          Unfollow
        {:else if isFollowing}
          Following
        {:else}
          Follow
        {/if}
      </button>

      <div class="tooltip tooltip-bottom" data-tip={isMuted ? 'Unmute' : 'Mute user'}>
        <button
          type="button"
          disabled={pending}
          class={isMuted ? 'btn btn-ghost btn-sm text-error' : 'btn btn-ghost btn-sm text-base-content/50'}
          onclick={isMuted ? confirmMute : openMuteDialog}
          aria-label={isMuted ? 'Unmute user' : 'Mute user'}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            {#if isMuted}
              <path d="M18 6L6 18M6 6l12 12" stroke-linecap="round" />
            {:else}
              <circle cx="12" cy="12" r="1" /><circle cx="19" cy="12" r="1" /><circle cx="5" cy="12" r="1" />
            {/if}
          </svg>
        </button>
      </div>

      {#if error}
        <span class="text-xs text-error">{error}</span>
      {/if}
    {:else}
      <span class="text-sm text-base-content/50">
        <a href="/login" class="text-primary hover:text-primary/80">Sign in</a> to follow
      </span>
    {/if}
  </div>
{/if}

<!-- Mute confirm dialog -->
<dialog bind:this={muteDialogEl} class="modal">
  <div class="modal-box">
    <h3 class="font-bold text-lg">Mute this person?</h3>
    <p class="py-4 text-base-content/70">Their content will be hidden across Highlighter.</p>
    <div class="modal-action">
      <form method="dialog">
        <button class="btn btn-ghost">Cancel</button>
      </form>
      <button class="btn btn-error" onclick={confirmMute}>Mute</button>
    </div>
  </div>
  <form method="dialog" class="modal-backdrop">
    <button>close</button>
  </form>
</dialog>
