<script lang="ts">
  import type { RoomSummary } from '$lib/ndk/groups';

  let {
    room,
    signedIn,
    joined,
    checkingMembership = false,
    joinPending = false,
    joinRequested = false,
    joinNotice = '',
    joinError = '',
    onJoin,
    onShare
  }: {
    room: RoomSummary;
    signedIn: boolean;
    joined: boolean;
    checkingMembership?: boolean;
    joinPending?: boolean;
    joinRequested?: boolean;
    joinNotice?: string;
    joinError?: string;
    onJoin?: () => void | Promise<void>;
    onShare?: () => void;
  } = $props();

  function accessLabel(access: RoomSummary['access']): string {
    return access === 'open' ? 'Open to join' : 'Invite only';
  }

  function visibilityLabel(visibility: RoomSummary['visibility']): string {
    return visibility === 'public' ? 'Public preview' : 'Private inside';
  }

  function titleForState(): string {
    if (joined) return 'You can jump straight into the shelf.';
    if (checkingMembership) return 'Checking your access.';
    if (joinRequested) return room.access === 'open' ? 'Your join request is in.' : 'Your request is waiting.';
    if (!signedIn) return room.access === 'open' ? 'Join this room.' : 'Request access to this room.';
    return room.access === 'open' ? 'Join the conversation here.' : 'Ask to join this room.';
  }

  function bodyForState(): string {
    if (joined) {
      return 'Share a new piece, save the lines worth talking about, and add your voice to the room.';
    }

    if (checkingMembership) {
      return 'We are checking whether this room already has you on the member list.';
    }

    if (joinRequested) {
      return room.access === 'open'
        ? 'This page will update as soon as the room adds you.'
        : 'A moderator can let you in as soon as they review the request.';
    }

    if (!signedIn) {
      return room.access === 'open'
        ? 'Create a profile to join, save highlights, and share your own picks.'
        : 'Create a profile first so you can ask to join and come back into the room.';
    }

    return room.access === 'open'
      ? 'Join now and you can start sharing into the shelf right away.'
      : 'This room is invite-only. Send a request and a moderator can open the door.';
  }

  function primaryActionLabel(): string {
    if (joined) return 'Share something';
    if (joinPending) return room.access === 'open' ? 'Joining...' : 'Sending...';
    if (joinRequested) return 'Request sent';
    if (!signedIn) return 'Set up a profile';
    return room.access === 'open' ? 'Join this room' : 'Request to join';
  }
</script>

<aside class="membership-panel">
  <div class="membership-badges">
    <span class="badge badge-ghost">{accessLabel(room.access)}</span>
    <span class="badge badge-ghost">{visibilityLabel(room.visibility)}</span>
  </div>

  <p class="panel-label">{joined ? 'Member' : 'Join'}</p>
  <h2>{titleForState()}</h2>
  <p class="membership-copy">{bodyForState()}</p>

  <div class="membership-actions">
    {#if joined}
      <button class="btn btn-primary btn-sm" type="button" onclick={() => onShare?.()}>{primaryActionLabel()}</button>
    {:else if checkingMembership}
      <span class="badge badge-ghost">Checking access...</span>
    {:else if signedIn}
      <button
        class="btn btn-primary btn-sm"
        type="button"
        disabled={joinPending || joinRequested}
        onclick={() => void onJoin?.()}
      >
        {primaryActionLabel()}
      </button>
    {:else}
      <a class="btn btn-primary btn-sm" href="/onboarding">{primaryActionLabel()}</a>
    {/if}

    <a class="btn btn-sm" href="/rooms">All rooms</a>
  </div>

  {#if joinError}
    <p class="status error">{joinError}</p>
  {:else if joinNotice && !joined}
    <p class="status">{joinNotice}</p>
  {/if}
</aside>

<style>
  .membership-panel {
    display: grid;
    gap: 0.95rem;
    padding: 1.1rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.25rem;
    background:
      radial-gradient(circle at top right, rgba(255, 103, 25, 0.12), transparent 42%),
      var(--surface);
    min-width: min(100%, 20rem);
  }

  .membership-badges,
  .membership-actions {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .panel-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.65rem;
    line-height: 1.08;
    letter-spacing: -0.03em;
  }

  .membership-copy,
  .status {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .status {
    font-size: 0.92rem;
  }

  .status.error {
    color: #a23a1d;
  }

  @media (max-width: 760px) {
    .membership-panel {
      min-width: 0;
    }
  }
</style>
