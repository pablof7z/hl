<script lang="ts">
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { User } from '$lib/ndk/ui/user';
  import {
    editRoomMetadata,
    addRoomMember,
    removeRoomMember,
    type CommunityAccess,
    type CommunityVisibility
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
  let genVisibility = $state<CommunityVisibility>('public');
  let genAccess = $state<CommunityAccess>('open');
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
  <div class="room-missing">
    <h1>Room not found</h1>
    <a href="/rooms" class="btn">Back to your rooms</a>
  </div>
{:else if currentUser && !isAdmin && adminFeed.events.size > 0}
  <div class="room-missing">
    <h1>Not authorized</h1>
    <p>Only room admins can access settings.</p>
    <a href="/r/{slug}" class="btn">Back to room</a>
  </div>
{:else}
  <div class="settings-wrap">
    <header class="settings-header">
      <a href="/r/{slug}" class="back-link">← {data.room.name}</a>
      <h1>Room settings</h1>
    </header>

    <!-- Tab strip -->
    <div class="tab-strip" role="tablist">
      {#each (['general', 'members', 'invite'] as const) as tab}
        <button
          role="tab"
          class="tab-btn"
          class:active={activeTab === tab}
          aria-selected={activeTab === tab}
          onclick={() => { activeTab = tab; }}
        >
          {tab === 'general' ? 'General' : tab === 'members' ? `Members · ${members.length}` : 'Invite'}
        </button>
      {/each}
    </div>

    <!-- General tab -->
    {#if activeTab === 'general'}
      <form class="settings-form" onsubmit={saveGeneral}>
        <section class="form-card">
          <fieldset class="fieldset">
            <legend class="fieldset-legend">Name</legend>
            <input class="field-input" bind:value={genName} maxlength="80" autocomplete="off" />
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Description</legend>
            <textarea class="field-input" bind:value={genAbout} rows="4" maxlength="280"></textarea>
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Cover image URL</legend>
            <input class="field-input" bind:value={genPicture} placeholder="https://…" inputmode="url" autocomplete="off" />
          </fieldset>
        </section>

        <section class="form-card">
          <fieldset class="fieldset">
            <legend class="fieldset-legend">Access</legend>
            <div class="option-row">
              <label class:active={genAccess === 'open'}>
                <input type="radio" bind:group={genAccess} value="open" disabled={genVisibility === 'private'} />
                <strong>Open</strong>
                <small>Anyone can join without an invite.</small>
              </label>
              <label class:active={genAccess === 'closed'}>
                <input type="radio" bind:group={genAccess} value="closed" />
                <strong>Closed</strong>
                <small>Membership requires approval or an invite.</small>
              </label>
            </div>
          </fieldset>

          <fieldset class="fieldset">
            <legend class="fieldset-legend">Visibility</legend>
            <div class="option-row">
              <label class:active={genVisibility === 'public'}>
                <input type="radio" bind:group={genVisibility} value="public" />
                <strong>Public</strong>
                <small>Room metadata can be browsed openly.</small>
              </label>
              <label class:active={genVisibility === 'private'}>
                <input type="radio" bind:group={genVisibility} value="private" />
                <strong>Private</strong>
                <small>Content is members-only and forces closed membership.</small>
              </label>
            </div>
          </fieldset>

          {#if genError}<p class="error-message">{genError}</p>{/if}
          {#if genSuccess}<p class="success-message">Settings saved.</p>{/if}

          <button class="btn btn-primary" type="submit" disabled={genSaving || !isAdmin}>
            {genSaving ? 'Saving…' : 'Save changes'}
          </button>
        </section>
      </form>
    {/if}

    <!-- Members tab -->
    {#if activeTab === 'members'}
      <div class="form-card members-card">
        {#if members.length === 0}
          <p class="empty-note">No members yet.</p>
        {:else}
          {#each members as m (m.pubkey)}
            {@const state = memberState(m.pubkey)}
            <div class="member-row">
              <span
                class="room-member-avatar"
                style:--mav-size="34px"
                style:--mav-ring={memberTint(m.colorIndex)}
              >
                <User.Root {ndk} pubkey={m.pubkey}>
                  <User.Avatar />
                </User.Root>
              </span>

              <div class="member-info">
                <User.Root {ndk} pubkey={m.pubkey}>
                  <span class="member-name"><User.Name field="displayName" /></span>
                  <span class="member-handle"><User.Handle /></span>
                </User.Root>
                {#if m.isAdmin}<span class="admin-badge">admin</span>{/if}
              </div>

              {#if isAdmin && m.pubkey !== currentUser?.pubkey}
                <div class="member-actions">
                  {#if state.error}<span class="action-error">{state.error}</span>{/if}
                  {#if m.isAdmin}
                    <button
                      class="btn-action"
                      disabled={state.pending}
                      onclick={() => handleRemoveAdmin(m.pubkey)}
                    >Remove admin</button>
                  {:else}
                    <button
                      class="btn-action"
                      disabled={state.pending}
                      onclick={() => handleMakeAdmin(m.pubkey)}
                    >Make admin</button>
                  {/if}
                  <button
                    class="btn-action btn-action--danger"
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

    <!-- Invite tab -->
    {#if activeTab === 'invite'}
      <form class="settings-form" onsubmit={handleInvite}>
        <section class="form-card">
          <fieldset class="fieldset">
            <legend class="fieldset-legend">Pubkey or npub</legend>
            <div class="invite-row">
              <input
                class="field-input"
                bind:value={inviteInput}
                placeholder="npub1… or hex pubkey"
                autocomplete="off"
                onblur={resolveInviteInput}
              />
              <button
                type="button"
                class="btn btn-ghost"
                onclick={resolveInviteInput}
              >Resolve</button>
            </div>
            <p class="fieldset-label">Paste a Nostr npub or hex pubkey. The user will be added immediately.</p>
          </fieldset>

          {#if invitePubkey}
            <div class="invite-preview">
              <User.Root {ndk} pubkey={invitePubkey}>
                <span class="room-member-avatar" style:--mav-size="36px" style:--mav-ring="var(--h-sage)">
                  <User.Avatar />
                </span>
                <div>
                  <div class="member-name"><User.Name field="displayName" /></div>
                  <div class="member-handle"><User.Handle /></div>
                </div>
              </User.Root>
            </div>
          {/if}

          {#if inviteError}<p class="error-message">{inviteError}</p>{/if}
          {#if inviteSuccess}<p class="success-message">Member added successfully.</p>{/if}

          <button class="btn btn-primary" type="submit" disabled={invitePending || !invitePubkey || !isAdmin}>
            {invitePending ? 'Adding…' : 'Add to room'}
          </button>
        </section>
      </form>
    {/if}
  </div>
{/if}

<style>
  .field-input {
    width: 100%;
    padding: 0.625rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
    transition: border-color 120ms ease;
    resize: vertical;
  }

  .field-input::placeholder { color: var(--muted); }
  .field-input:focus { border-color: var(--accent); }

  .settings-wrap {
    display: grid;
    gap: 1.5rem;
    padding: 2rem 0 3rem;
  }

  .settings-header {
    display: grid;
    gap: 0.35rem;
  }

  .back-link {
    color: var(--brand-accent);
    font-family: var(--font-sans);
    font-size: 0.85rem;
    font-weight: 500;
    text-decoration: none;
  }

  .back-link:hover { text-decoration: underline; }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.8rem, 4vw, 2.6rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .tab-strip {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border);
  }

  .tab-btn {
    padding: 0.6rem 1.1rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    font-family: var(--font-sans);
    font-size: 0.88rem;
    font-weight: 500;
    color: var(--muted);
    transition: color 120ms ease, border-color 120ms ease;
    margin-bottom: -1px;
  }

  .tab-btn.active {
    color: var(--text-strong);
    border-bottom-color: var(--brand-accent);
  }

  .settings-form {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.9fr);
    gap: 1rem;
    align-items: start;
  }

  @media (max-width: 860px) {
    .settings-form { grid-template-columns: 1fr; }
  }

  .form-card {
    display: grid;
    gap: 1rem;
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background: var(--surface);
  }

  .members-card {
    grid-template-columns: 1fr;
  }

  .fieldset {
    display: grid;
    gap: 0.45rem;
    border: none;
    padding: 0;
    margin: 0;
  }

  .fieldset-legend {
    font-family: var(--font-sans);
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .fieldset-label {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .option-row {
    display: grid;
    gap: 0.65rem;
  }

  .option-row label {
    display: grid;
    gap: 0.25rem;
    padding: 0.95rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface-soft);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .option-row label.active {
    border-color: rgba(255, 103, 25, 0.32);
    background: rgba(255, 103, 25, 0.05);
  }

  .option-row label:has(input:disabled) { opacity: 0.65; }

  .option-row small {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .option-row input { margin: 0; }

  .member-row {
    display: grid;
    grid-template-columns: 36px 1fr auto;
    gap: 12px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
  }

  .member-row:last-child { border-bottom: none; }

  .member-info {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .member-name {
    font-family: var(--font-sans);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink);
  }

  .member-handle {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 400;
    color: var(--ink-fade);
  }

  .admin-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--brand-accent);
    background: rgba(255, 103, 25, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .member-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .action-error {
    font-size: 11px;
    color: var(--pale-red-text);
  }

  .btn-action {
    padding: 5px 10px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    transition: background 120ms ease;
    white-space: nowrap;
  }

  .btn-action:hover { background: var(--border); }
  .btn-action:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn-action--danger {
    color: #c0392b;
    border-color: rgba(192, 57, 43, 0.25);
    background: rgba(192, 57, 43, 0.05);
  }

  .btn-action--danger:hover { background: rgba(192, 57, 43, 0.12); }

  .invite-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    align-items: center;
  }

  .invite-preview {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 1rem;
  }

  .error-message, .success-message {
    margin: 0;
    padding: 0.8rem 0.95rem;
    border-radius: 0.95rem;
    font-size: 0.88rem;
    line-height: 1.55;
  }

  .error-message {
    background: var(--pale-red);
    color: var(--pale-red-text);
  }

  .success-message {
    background: rgba(25, 160, 90, 0.1);
    color: #176b42;
  }

  .room-missing, .empty-note {
    padding: 60px 0;
    text-align: center;
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 14px;
  }

  .room-missing h1 {
    font-family: var(--font-serif);
    font-size: 32px;
    font-weight: 400;
    color: var(--ink);
    margin: 0 0 12px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    padding: 10px 20px;
    background: var(--ink);
    color: var(--surface);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: var(--radius);
    transition: background 200ms ease;
  }

  .btn:hover { background: var(--brand-accent); }

  /* room-member-avatar ring pattern (mirrors room page) */
  :global(.room-member-avatar) {
    display: inline-flex;
    width: var(--mav-size, 34px);
    height: var(--mav-size, 34px);
    border-radius: 50%;
    outline: 2.5px solid var(--mav-ring, transparent);
    outline-offset: 1px;
    flex-shrink: 0;
    overflow: hidden;
  }
</style>
