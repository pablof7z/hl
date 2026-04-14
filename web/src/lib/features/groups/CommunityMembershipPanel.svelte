<script lang="ts">
  import type { CommunitySummary } from '$lib/ndk/groups';

  let {
    community,
    signedIn,
    joined,
    checkingMembership = false,
    joinPending = false,
    joinRequested = false,
    joinNotice = '',
    joinError = '',
    onJoin
  }: {
    community: CommunitySummary;
    signedIn: boolean;
    joined: boolean;
    checkingMembership?: boolean;
    joinPending?: boolean;
    joinRequested?: boolean;
    joinNotice?: string;
    joinError?: string;
    onJoin?: () => void | Promise<void>;
  } = $props();

  function accessLabel(access: CommunitySummary['access']): string {
    return access === 'open' ? 'Open to join' : 'Invite only';
  }

  function visibilityLabel(visibility: CommunitySummary['visibility']): string {
    return visibility === 'public' ? 'Public preview' : 'Private inside';
  }

  function titleForState(): string {
    if (joined) return 'You can jump straight into the shelf.';
    if (checkingMembership) return 'Checking your access.';
    if (joinRequested) return community.access === 'open' ? 'Your join request is in.' : 'Your request is waiting.';
    if (!signedIn) return community.access === 'open' ? 'Join this community.' : 'Request access to this community.';
    return community.access === 'open' ? 'Join the conversation here.' : 'Ask to join this room.';
  }

  function bodyForState(): string {
    if (joined) {
      return 'Share a new piece, save the lines worth talking about, and add your voice to the room.';
    }

    if (checkingMembership) {
      return 'We are checking whether this community already has you on the member list.';
    }

    if (joinRequested) {
      return community.access === 'open'
        ? 'This page will update as soon as the community adds you.'
        : 'A moderator can let you in as soon as they review the request.';
    }

    if (!signedIn) {
      return community.access === 'open'
        ? 'Create a profile to join, save highlights, and share your own picks.'
        : 'Create a profile first so you can ask to join and come back into the room.';
    }

    return community.access === 'open'
      ? 'Join now and you can start sharing into the shelf right away.'
      : 'This community is invite-only. Send a request and a moderator can open the door.';
  }

  function primaryActionLabel(): string {
    if (joined) return 'Share something';
    if (joinPending) return community.access === 'open' ? 'Joining...' : 'Sending...';
    if (joinRequested) return 'Request sent';
    if (!signedIn) return 'Set up a profile';
    return community.access === 'open' ? 'Join this community' : 'Request to join';
  }
</script>

<aside class="membership-panel">
  <div class="membership-badges">
    <span>{accessLabel(community.access)}</span>
    <span>{visibilityLabel(community.visibility)}</span>
  </div>

  <p class="panel-label">{joined ? 'Member' : 'Join'}</p>
  <h2>{titleForState()}</h2>
  <p class="membership-copy">{bodyForState()}</p>

  <div class="membership-actions">
    {#if joined}
      <a class="primary-action" href="#share-artifact">{primaryActionLabel()}</a>
    {:else if checkingMembership}
      <span class="pending-pill">Checking access...</span>
    {:else if signedIn}
      <button
        class="primary-action"
        type="button"
        disabled={joinPending || joinRequested}
        onclick={() => void onJoin?.()}
      >
        {primaryActionLabel()}
      </button>
    {:else}
      <a class="primary-action" href="/onboarding">{primaryActionLabel()}</a>
    {/if}

    <a class="secondary-action" href="/community">All communities</a>
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
    border: 1px solid var(--border);
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

  .membership-badges span,
  .pending-pill {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.7rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 700;
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

  .primary-action,
  .secondary-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.6rem;
    padding: 0 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    font-weight: 700;
  }

  .primary-action {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  button.primary-action:disabled {
    cursor: default;
    opacity: 0.7;
  }

  .secondary-action {
    background: var(--surface);
    color: var(--text);
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
