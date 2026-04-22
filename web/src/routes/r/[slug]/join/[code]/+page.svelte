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
  <div class="grid gap-3 justify-items-center py-20 px-4 text-center">
    <h1 class="m-0 text-base-content font-serif text-[1.8rem] tracking-[-0.02em]">This room isn't here.</h1>
    <p class="m-0 text-base-content/50">The link might be broken, or the room may have been removed.</p>
    <a href="/" class="btn">Back to Highlighter</a>
  </div>
{:else}
  <section class="grid place-items-center pt-12 pb-16 px-4">
    <div class="grid gap-0 w-full max-w-[32rem] bg-base-100 border border-base-300 rounded-[1.25rem] overflow-hidden">
      {#if room.picture}
        <div
          class="aspect-[3/1] bg-cover bg-center bg-base-200 border-b border-base-300"
          style:background-image="url({room.picture})"
        ></div>
      {/if}

      <div class="grid gap-4 p-[1.75rem_1.75rem_2rem]">
        <h1 class="m-0 text-base-content font-serif text-[clamp(2rem,5vw,2.8rem)] leading-[1.05] tracking-[-0.03em]">{room.name}</h1>

        {#if inviterPubkey}
          <div class="inline-flex items-center gap-2 text-base-content/50 text-[0.92rem]">
            <User.Root ndk={ndk} pubkey={inviterPubkey}>
              <span class="inline-flex w-7 h-7 rounded-full overflow-hidden shrink-0 [&_:global(img)]:w-full [&_:global(img)]:h-full [&_:global(img)]:object-cover">
                <User.Avatar />
              </span>
              <span class="text-base-content">
                From <User.Name field="displayName" />
              </span>
            </User.Root>
          </div>
        {/if}

        {#if room.about}
          <p class="m-0 text-base-content text-[1.02rem] leading-[1.55]">{room.about}</p>
        {/if}

        {#if !isPrivate}
          <div class="text-base-content/50 text-[0.85rem] font-mono tracking-[0.04em]">
            {liveMemberPubkeys.length} member{liveMemberPubkeys.length === 1 ? '' : 's'}
          </div>
        {/if}

        <div class="flex gap-[0.6rem] items-center pt-2">
          {#if accepted && !alreadyMember}
            <span class="text-base-content/50 italic">Joining…</span>
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
          <p class="m-0 px-[0.85rem] py-[0.7rem] rounded-[0.65rem] bg-error/10 text-error text-[0.9rem] leading-[1.5]">{acceptError}</p>
        {/if}

        {#if isPrivate && !alreadyMember}
          <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">
            This is a members-only room. You'll see what's inside after you accept.
          </p>
        {:else if isOpen && !alreadyMember}
          <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">
            This room is open — you can also enter without this link.
          </p>
        {/if}
      </div>
    </div>
  </section>
{/if}
