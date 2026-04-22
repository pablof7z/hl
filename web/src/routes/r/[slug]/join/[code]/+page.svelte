<script lang="ts">
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { acceptInviteCode } from '$lib/ndk/groups';
  import { setPendingInvite, clearPendingInvite } from '$lib/features/groups/pendingInvite';
  import { User } from '$lib/ndk/ui/user';
  import type { PageData } from './$types';

  const KIND_GROUP_ADMIN_CREATE_INVITE = 9009;

  let { data }: { data: PageData } = $props();

  const room = $derived(data.room);
  const slug = $derived(room?.id ?? '');
  const code = $derived(data.code);
  const currentUser = $derived(ndk.$currentUser);
  const isPrivate = $derived(room?.visibility === 'private');
  const isOpen = $derived(room?.access === 'open');

  // Watch the group's live member list to detect if this signer is already in.
  const memberFeed = ndk.$subscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupMembers], '#d': [slug] }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const liveMemberPubkeys = $derived.by<string[]>(() => {
    const events = [...memberFeed.events].sort(
      (a, b) => (b.created_at ?? 0) - (a.created_at ?? 0)
    );
    if (events.length === 0) return (room?.members ?? []).map((m) => m.pubkey);
    return events[0]
      .getMatchingTags('p')
      .map((tag) => tag[1])
      .filter(Boolean);
  });

  const alreadyMember = $derived(
    !!currentUser && liveMemberPubkeys.includes(currentUser.pubkey)
  );

  // Look up the inviter (the admin who minted this code) when the relay will
  // serve it. Public rooms will, hidden ones typically won't — that's fine.
  const inviterFeed = ndk.$subscribe(() => {
    if (!browser || !slug || !code) return undefined;
    return {
      filters: [
        {
          kinds: [KIND_GROUP_ADMIN_CREATE_INVITE],
          '#h': [slug]
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const inviterPubkey = $derived.by<string | null>(() => {
    for (const event of inviterFeed.events) {
      const codes = event.getMatchingTags('code').map((t) => t[1]);
      if (codes.includes(code)) return event.pubkey;
    }
    return null;
  });

  let accepting = $state(false);
  let acceptError = $state('');
  let accepted = $state(false);

  // Once the signer appears in the member list after an accept, navigate into
  // the room. Running this as a reactive effect handles both paths: the
  // just-accepted flow and the "already a member" pre-existing case after
  // returning from onboarding.
  $effect(() => {
    if (!browser) return;
    if (!alreadyMember) return;
    if (!accepted) return;
    void goto(`/r/${slug}`, { invalidateAll: true });
  });

  async function handleAccept() {
    if (!slug || !code) return;
    if (!currentUser) return;
    try {
      accepting = true;
      acceptError = '';
      await ensureClientNdk();
      await acceptInviteCode(ndk, slug, code);
      accepted = true;
      clearPendingInvite();
      // If the member-list subscription hasn't caught up yet, just navigate.
      setTimeout(() => {
        if (!accepted) return;
        void goto(`/r/${slug}`, { invalidateAll: true });
      }, 1200);
    } catch (error) {
      acceptError = error instanceof Error ? error.message : 'Could not accept the invitation.';
    } finally {
      accepting = false;
    }
  }

  function handleSignInToAccept() {
    if (!slug || !code) return;
    setPendingInvite({ groupId: slug, code });
    void goto('/onboarding');
  }
</script>

<svelte:head>
  <title>
    {room ? `Join ${room.name} · Highlighter` : 'Invitation · Highlighter'}
  </title>
</svelte:head>

{#if !room}
  <div class="empty-state">
    <h1>This room isn't here.</h1>
    <p>The link might be broken, or the room may have been removed.</p>
    <a href="/" class="btn">Back to Highlighter</a>
  </div>
{:else}
  <section class="join-page">
    <div class="calling-card">
      {#if room.picture}
        <div class="cover" style:background-image="url({room.picture})"></div>
      {/if}

      <div class="card-body">
        <div class="kicker">— an invitation</div>
        <h1 class="room-name">{room.name}</h1>

        {#if inviterPubkey}
          <div class="inviter">
            <User.Root ndk={ndk} pubkey={inviterPubkey}>
              <span class="inviter-avatar">
                <User.Avatar />
              </span>
              <span class="inviter-text">
                From <User.Name field="displayName" />
              </span>
            </User.Root>
          </div>
        {/if}

        {#if room.about}
          <p class="about">{room.about}</p>
        {/if}

        {#if !isPrivate}
          <div class="room-stats">
            {liveMemberPubkeys.length} member{liveMemberPubkeys.length === 1 ? '' : 's'}
          </div>
        {/if}

        <div class="actions">
          {#if accepted && !alreadyMember}
            <span class="status-ok">Joining…</span>
          {:else if alreadyMember}
            <a class="btn btn-primary" href="/r/{slug}">Enter the room</a>
          {:else if currentUser}
            <button
              type="button"
              class="btn btn-primary"
              onclick={handleAccept}
              disabled={accepting}
            >
              {accepting ? 'Accepting…' : 'Accept invitation'}
            </button>
          {:else}
            <button
              type="button"
              class="btn btn-primary"
              onclick={handleSignInToAccept}
            >
              Sign in to accept
            </button>
          {/if}
        </div>

        {#if acceptError}
          <p class="error-banner">{acceptError}</p>
        {/if}

        {#if isPrivate && !alreadyMember}
          <p class="private-note">
            This is a members-only room. You'll see what's inside after you accept.
          </p>
        {:else if isOpen && !alreadyMember}
          <p class="quiet-note">
            This room is open — you can also enter without this link.
          </p>
        {/if}
      </div>
    </div>
  </section>
{/if}

<style>
  .join-page {
    display: grid;
    place-items: center;
    padding: 3rem 1rem 4rem;
  }

  .calling-card {
    display: grid;
    gap: 0;
    width: 100%;
    max-width: 32rem;
    background: var(--surface);
    border: 1px solid var(--color-base-300);
    border-radius: 1.25rem;
    overflow: hidden;
  }

  .cover {
    aspect-ratio: 3 / 1;
    background-size: cover;
    background-position: center;
    background-color: var(--surface-soft);
    border-bottom: 1px solid var(--color-base-300);
  }

  .card-body {
    display: grid;
    gap: 1rem;
    padding: 1.75rem 1.75rem 2rem;
  }

  .kicker {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .room-name {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 5vw, 2.8rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .inviter {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--muted);
    font-size: 0.92rem;
  }

  .inviter-avatar {
    display: inline-flex;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    overflow: hidden;
    flex-shrink: 0;
  }

  .inviter-avatar :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .inviter-text {
    color: var(--text-strong);
  }

  .about {
    margin: 0;
    color: var(--text-strong);
    font-size: 1.02rem;
    line-height: 1.55;
  }

  .room-stats {
    color: var(--muted);
    font-size: 0.85rem;
    font-family: var(--font-mono);
    letter-spacing: 0.04em;
  }

  .actions {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    padding-top: 0.5rem;
  }

  .btn-primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  .status-ok {
    color: var(--muted);
    font-style: italic;
  }

  .error-banner {
    margin: 0;
    padding: 0.7rem 0.85rem;
    border-radius: 0.65rem;
    background: var(--pale-red);
    color: var(--pale-red-text);
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .private-note,
  .quiet-note {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
    line-height: 1.55;
  }

  .empty-state {
    display: grid;
    gap: 0.75rem;
    justify-items: center;
    padding: 5rem 1rem;
    text-align: center;
  }

  .empty-state h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.8rem;
    letter-spacing: -0.02em;
  }

  .empty-state p {
    margin: 0;
    color: var(--muted);
  }
</style>
