<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { User } from '$lib/ndk/ui/user';
  import {
    editRoomMetadata,
    addRoomMember,
    removeRoomMember,
    type RoomAccess,
    type RoomVisibility
  } from '$lib/ndk/groups';
  import { memberTint } from '$lib/features/room/utils/colors';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const slug = $derived(data.room?.id ?? '');
  const currentUser = $derived(ndk.$currentUser);

  // Client-side subscriptions to pick up relay updates after admin actions
  const adminFeed = ndk.$subscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupAdmins], '#d': [slug] }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  const memberFeed = ndk.$subscribe(() => {
    if (!browser || !slug) return undefined;
    return {
      filters: [{ kinds: [NDKKind.GroupMembers], '#d': [slug] }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  // Derive live admin and member pubkeys from subscriptions (falling back to SSR data)
  const liveMemberPubkeys = $derived.by<string[]>(() => {
    const events = [...memberFeed.events].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
    if (events.length > 0) {
      return events[0].getMatchingTags('p').map((t) => t[1]).filter(Boolean);
    }
    return (data.room?.members ?? []).map((m) => m.pubkey);
  });

  const liveAdminPubkeys = $derived.by<string[]>(() => {
    const events = [...adminFeed.events].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
    if (events.length > 0) {
      return events[0].getMatchingTags('p').map((t) => t[1]).filter(Boolean);
    }
    return data.room?.adminPubkeys ?? [];
  });

  const isAdmin = $derived(
    !!currentUser && liveAdminPubkeys.includes(currentUser.pubkey)
  );

  // Members list with color index and admin flag
  const members = $derived(
    liveMemberPubkeys.map((pubkey, index) => ({
      pubkey,
      colorIndex: (index % 6) + 1,
      isAdmin: liveAdminPubkeys.includes(pubkey)
    }))
  );

  // --- Tab state ---
  let activeTab = $state<'general' | 'members' | 'invite'>('general');

  // --- General tab ---
  let genName = $state(data.room?.name ?? '');
  let genAbout = $state('');
  let genPicture = $state('');
  let genVisibility = $state<RoomVisibility>('public');
  let genAccess = $state<RoomAccess>('open');
  let genSaving = $state(false);
  let genError = $state('');
  let genSuccess = $state(false);

  $effect(() => {
    if (genVisibility === 'private' && genAccess === 'open') genAccess = 'closed';
  });

  async function saveGeneral(e: SubmitEvent) {
    e.preventDefault();
    if (!genName.trim()) { genError = 'Name is required.'; return; }
    genSaving = true; genError = ''; genSuccess = false;
    try {
      await ensureClientNdk();
      await editRoomMetadata(ndk, slug, {
        name: genName,
        about: genAbout,
        picture: genPicture,
        visibility: genVisibility,
        access: genAccess
      });
      genSuccess = true;
    } catch (err) {
      genError = err instanceof Error ? err.message : 'Could not save settings.';
    } finally {
      genSaving = false;
    }
  }

  // --- Members tab ---
  let memberAction = $state<Record<string, { pending: boolean; error: string }>>({});

  function memberState(pubkey: string) {
    return memberAction[pubkey] ?? { pending: false, error: '' };
  }

  async function handleRemove(pubkey: string) {
    memberAction[pubkey] = { pending: true, error: '' };
    try {
      await ensureClientNdk();
      await removeRoomMember(ndk, slug, pubkey);
    } catch (err) {
      memberAction[pubkey] = { pending: false, error: err instanceof Error ? err.message : 'Failed.' };
    } finally {
      if (memberAction[pubkey]) memberAction[pubkey].pending = false;
    }
  }

  async function handleMakeAdmin(pubkey: string) {
    memberAction[pubkey] = { pending: true, error: '' };
    try {
      await ensureClientNdk();
      await addRoomMember(ndk, slug, pubkey, 'admin');
    } catch (err) {
      memberAction[pubkey] = { pending: false, error: err instanceof Error ? err.message : 'Failed.' };
    } finally {
      if (memberAction[pubkey]) memberAction[pubkey].pending = false;
    }
  }

  async function handleRemoveAdmin(pubkey: string) {
    memberAction[pubkey] = { pending: true, error: '' };
    try {
      await ensureClientNdk();
      // Remove then re-add without admin role so they stay as a member
      await removeRoomMember(ndk, slug, pubkey);
      await addRoomMember(ndk, slug, pubkey);
    } catch (err) {
      memberAction[pubkey] = { pending: false, error: err instanceof Error ? err.message : 'Failed.' };
    } finally {
      if (memberAction[pubkey]) memberAction[pubkey].pending = false;
    }
  }

  // --- Invite tab ---
  let inviteInput = $state('');
  let invitePubkey = $state('');
  let inviteError = $state('');
  let inviteSuccess = $state(false);
  let invitePending = $state(false);

  async function resolveInviteInput() {
    inviteError = '';
    invitePubkey = '';
    const raw = inviteInput.trim();
    if (!raw) return;
    try {
      const user = ndk.getUser(raw.startsWith('npub') ? { npub: raw } : { pubkey: raw });
      invitePubkey = user.pubkey;
    } catch {
      inviteError = 'Enter a valid npub or hex pubkey.';
    }
  }

  async function handleInvite(e: SubmitEvent) {
    e.preventDefault();
    if (!invitePubkey) { inviteError = 'Resolve a valid pubkey first.'; return; }
    invitePending = true; inviteError = ''; inviteSuccess = false;
    try {
      await ensureClientNdk();
      await addRoomMember(ndk, slug, invitePubkey);
      inviteSuccess = true;
      inviteInput = '';
      invitePubkey = '';
    } catch (err) {
      inviteError = err instanceof Error ? err.message : 'Could not add member.';
    } finally {
      invitePending = false;
    }
  }
</script>

<svelte:head>
  <title>Room Settings — {data.room?.name ?? slug}</title>
</svelte:head>

{#if !data.room}
  <div class="py-15 text-center text-base-content/80 font-sans text-[14px]">
    <h1 class="font-serif text-[32px] font-normal text-base-content m-0 mb-3">Room not found</h1>
    <a href="/rooms" class="inline-flex items-center px-5 py-2.5 bg-base-content text-base-100 font-sans text-[13px] font-medium no-underline rounded hover:bg-primary transition-colors duration-200">Back to your rooms</a>
  </div>
{:else if currentUser && !isAdmin && adminFeed.events.length > 0}
  <div class="py-15 text-center text-base-content/80 font-sans text-[14px]">
    <h1 class="font-serif text-[32px] font-normal text-base-content m-0 mb-3">Not authorized</h1>
    <p class="m-0 mb-3">Only room admins can access settings.</p>
    <a href="/r/{slug}" class="inline-flex items-center px-5 py-2.5 bg-base-content text-base-100 font-sans text-[13px] font-medium no-underline rounded hover:bg-primary transition-colors duration-200">Back to room</a>
  </div>
{:else}
  <div class="grid gap-6 py-8 pb-12">
    <header class="grid gap-[0.35rem]">
      <a href="/r/{slug}" class="text-primary font-sans text-[0.85rem] font-medium no-underline hover:underline">← {data.room.name}</a>
      <h1 class="m-0 text-base-content font-serif text-[clamp(1.8rem,4vw,2.6rem)] leading-[1.05] tracking-[-0.03em]">Room settings</h1>
    </header>

    <div class="flex gap-0 border-b border-base-300" role="tablist">
      {#each (['general', 'members', 'invite'] as const) as tab}
        <button
          role="tab"
          class="px-[1.1rem] py-[0.6rem] bg-none border-none border-b-2 border-transparent -mb-px cursor-pointer font-sans text-[0.88rem] font-medium text-base-content/50 transition-[color,border-color] duration-[120ms] {activeTab === tab ? 'text-base-content border-b-primary' : ''}"
          aria-selected={activeTab === tab}
          onclick={() => { activeTab = tab; }}
        >
          {tab === 'general' ? 'General' : tab === 'members' ? `Members · ${members.length}` : 'Invite'}
        </button>
      {/each}
    </div>

    {#if activeTab === 'general'}
      <form class="grid grid-cols-[minmax(0,1.2fr)_minmax(0,0.9fr)] gap-4 items-start max-[860px]:grid-cols-1" onsubmit={saveGeneral}>
        <section class="grid gap-4 p-5 border border-base-300 rounded-[1.35rem] bg-base-100">
          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Name</legend>
            <input class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-200 text-base-content text-[0.875rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] focus:border-primary resize-y placeholder:text-base-content/50" bind:value={genName} maxlength="80" autocomplete="off" />
          </fieldset>

          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Description</legend>
            <textarea class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-200 text-base-content text-[0.875rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] focus:border-primary resize-y placeholder:text-base-content/50" bind:value={genAbout} rows="4" maxlength="280"></textarea>
          </fieldset>

          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Cover image URL</legend>
            <input class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-200 text-base-content text-[0.875rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] focus:border-primary resize-y placeholder:text-base-content/50" bind:value={genPicture} placeholder="https://…" inputmode="url" autocomplete="off" />
          </fieldset>
        </section>

        <section class="grid gap-4 p-5 border border-base-300 rounded-[1.35rem] bg-base-100">
          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Access</legend>
            <div class="grid gap-[0.65rem]">
              <label class="grid gap-[0.25rem] p-[0.95rem_1rem] border border-base-300 rounded-2xl bg-base-200 cursor-pointer transition-[border-color,background] duration-[120ms] {genAccess === 'open' ? 'border-primary/30 bg-primary/5' : ''}">
                <input type="radio" bind:group={genAccess} value="open" disabled={genVisibility === 'private'} class="m-0" />
                <strong>Open</strong>
                <small class="text-base-content/50 text-[0.8rem]">Anyone can join without an invite.</small>
              </label>
              <label class="grid gap-[0.25rem] p-[0.95rem_1rem] border border-base-300 rounded-2xl bg-base-200 cursor-pointer transition-[border-color,background] duration-[120ms] {genAccess === 'closed' ? 'border-primary/30 bg-primary/5' : ''}">
                <input type="radio" bind:group={genAccess} value="closed" class="m-0" />
                <strong>Closed</strong>
                <small class="text-base-content/50 text-[0.8rem]">Membership requires approval or an invite.</small>
              </label>
            </div>
          </fieldset>

          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Visibility</legend>
            <div class="grid gap-[0.65rem]">
              <label class="grid gap-[0.25rem] p-[0.95rem_1rem] border border-base-300 rounded-2xl bg-base-200 cursor-pointer transition-[border-color,background] duration-[120ms] {genVisibility === 'public' ? 'border-primary/30 bg-primary/5' : ''}">
                <input type="radio" bind:group={genVisibility} value="public" class="m-0" />
                <strong>Public</strong>
                <small class="text-base-content/50 text-[0.8rem]">Room metadata can be browsed openly.</small>
              </label>
              <label class="grid gap-[0.25rem] p-[0.95rem_1rem] border border-base-300 rounded-2xl bg-base-200 cursor-pointer transition-[border-color,background] duration-[120ms] {genVisibility === 'private' ? 'border-primary/30 bg-primary/5' : ''}">
                <input type="radio" bind:group={genVisibility} value="private" class="m-0" />
                <strong>Private</strong>
                <small class="text-base-content/50 text-[0.8rem]">Content is members-only and forces closed membership.</small>
              </label>
            </div>
          </fieldset>

          {#if genError}<p class="m-0 p-[0.8rem_0.95rem] rounded-[0.95rem] text-[0.88rem] leading-[1.55] bg-error/10 text-error">{genError}</p>{/if}
          {#if genSuccess}<p class="m-0 p-[0.8rem_0.95rem] rounded-[0.95rem] text-[0.88rem] leading-[1.55] bg-success/10 text-success">Settings saved.</p>{/if}

          <button class="inline-flex items-center px-5 py-2.5 bg-primary text-primary-content font-sans text-[13px] font-medium no-underline rounded transition-[background] duration-200 disabled:opacity-50 disabled:cursor-not-allowed" type="submit" disabled={genSaving || !isAdmin}>
            {genSaving ? 'Saving…' : 'Save changes'}
          </button>
        </section>
      </form>
    {/if}

    {#if activeTab === 'members'}
      <div class="grid gap-4 p-5 border border-base-300 rounded-[1.35rem] bg-base-100">
        {#if members.length === 0}
          <p class="py-15 text-center text-base-content/80 font-sans text-[14px] m-0">No members yet.</p>
        {:else}
          {#each members as m (m.pubkey)}
            {@const state = memberState(m.pubkey)}
            <div class="grid grid-cols-[36px_1fr_auto] gap-3 items-center py-[10px] border-b border-dotted border-black/8 last:border-b-0">
              <span
                class="room-member-avatar"
                style:--mav-size="34px"
                style:--mav-ring={memberTint(m.colorIndex)}
              >
                <User.Root {ndk} pubkey={m.pubkey}>
                  <User.Avatar />
                </User.Root>
              </span>

              <div class="flex items-center gap-2 flex-wrap">
                <User.Root {ndk} pubkey={m.pubkey}>
                  <span class="font-sans text-[13.5px] font-semibold text-base-content"><User.Name field="displayName" /></span>
                  <span class="font-mono text-[12px] font-normal text-base-content/50"><User.Handle /></span>
                </User.Root>
                {#if m.isAdmin}<span class="font-mono text-[10px] tracking-[0.1em] uppercase text-primary bg-primary/10 px-[6px] py-[2px] rounded-[4px]">admin</span>{/if}
              </div>

              {#if isAdmin && m.pubkey !== currentUser?.pubkey}
                <div class="flex gap-[6px] items-center">
                  {#if state.error}<span class="text-[11px] text-error">{state.error}</span>{/if}
                  {#if m.isAdmin}
                    <button
                      class="px-[10px] py-[5px] font-sans text-[12px] font-medium bg-base-200 border border-base-300 rounded-[6px] cursor-pointer transition-[background] duration-[120ms] hover:bg-base-300 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                      disabled={state.pending}
                      onclick={() => handleRemoveAdmin(m.pubkey)}
                    >Remove admin</button>
                  {:else}
                    <button
                      class="px-[10px] py-[5px] font-sans text-[12px] font-medium bg-base-200 border border-base-300 rounded-[6px] cursor-pointer transition-[background] duration-[120ms] hover:bg-base-300 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                      disabled={state.pending}
                      onclick={() => handleMakeAdmin(m.pubkey)}
                    >Make admin</button>
                  {/if}
                  <button
                    class="px-[10px] py-[5px] font-sans text-[12px] font-medium text-[#c0392b] border border-[rgba(192,57,43,0.25)] bg-[rgba(192,57,43,0.05)] rounded-[6px] cursor-pointer transition-[background] duration-[120ms] hover:bg-[rgba(192,57,43,0.12)] disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                    disabled={state.pending}
                    onclick={() => handleRemove(m.pubkey)}
                  >Remove</button>
                </div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}

    {#if activeTab === 'invite'}
      <div class="grid grid-cols-[minmax(0,1.2fr)_minmax(0,0.9fr)] gap-4 items-start max-[860px]:grid-cols-1">
        <section class="grid gap-4 p-5 border border-base-300 rounded-[1.35rem] bg-base-100">
          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Invite links</legend>
            <p class="m-0 text-[0.8rem] text-base-content/50 leading-[1.5]">
              Create invite links for people you want in the room. Each link
              works once.
            </p>
            <a class="inline-flex items-center px-5 py-2.5 bg-primary text-primary-content font-sans text-[13px] font-medium no-underline rounded transition-[background] duration-200" href="/r/{slug}/invite">Manage invite links</a>
          </fieldset>
        </section>

        <form class="grid gap-4 p-5 border border-base-300 rounded-[1.35rem] bg-base-100" onsubmit={handleInvite}>
          <fieldset class="grid gap-[0.45rem] border-none p-0 m-0">
            <legend class="font-sans text-[0.78rem] font-bold tracking-[0.07em] uppercase text-base-content/50">Add someone by pubkey</legend>
            <div class="grid grid-cols-[1fr_auto] gap-2 items-center">
              <input
                class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-200 text-base-content text-[0.875rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] focus:border-primary resize-y placeholder:text-base-content/50"
                bind:value={inviteInput}
                placeholder="npub1… or hex pubkey"
                autocomplete="off"
                onblur={resolveInviteInput}
              />
              <button
                type="button"
                class="px-[10px] py-[5px] font-sans text-[12px] font-medium bg-base-200 border border-base-300 rounded-[6px] cursor-pointer transition-[background] duration-[120ms] hover:bg-base-300 whitespace-nowrap"
                onclick={resolveInviteInput}
              >Resolve</button>
            </div>
            <p class="m-0 text-[0.8rem] text-base-content/50 leading-[1.5]">Paste a Nostr npub or hex pubkey to add a member directly, without an invite link.</p>
          </fieldset>

          {#if invitePubkey}
            <div class="flex items-center gap-3 p-[10px_12px] bg-base-200 border border-base-300 rounded-2xl">
              <User.Root {ndk} pubkey={invitePubkey}>
                <span class="room-member-avatar" style:--mav-size="36px" style:--mav-ring="var(--h-sage)">
                  <User.Avatar />
                </span>
                <div>
                  <div class="font-sans text-[13.5px] font-semibold text-base-content"><User.Name field="displayName" /></div>
                  <div class="font-mono text-[12px] font-normal text-base-content/50"><User.Handle /></div>
                </div>
              </User.Root>
            </div>
          {/if}

          {#if inviteError}<p class="m-0 p-[0.8rem_0.95rem] rounded-[0.95rem] text-[0.88rem] leading-[1.55] bg-error/10 text-error">{inviteError}</p>{/if}
          {#if inviteSuccess}<p class="m-0 p-[0.8rem_0.95rem] rounded-[0.95rem] text-[0.88rem] leading-[1.55] bg-success/10 text-success">Member added successfully.</p>{/if}

          <button class="inline-flex items-center px-5 py-2.5 bg-primary text-primary-content font-sans text-[13px] font-medium no-underline rounded transition-[background] duration-200 disabled:opacity-50 disabled:cursor-not-allowed" type="submit" disabled={invitePending || !invitePubkey || !isAdmin}>
            {invitePending ? 'Adding…' : 'Add to room'}
          </button>
        </form>
      </div>
    {/if}
  </div>
{/if}
