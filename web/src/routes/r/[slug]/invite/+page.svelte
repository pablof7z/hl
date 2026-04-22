<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import {
    createInviteCodes,
    MAX_CODES_PER_INVITE_EVENT
  } from '$lib/ndk/groups';
  import {
    listInvites,
    recordInvite,
    recordInvites,
    deleteInvite,
    type InviteRecord
  } from '$lib/features/groups/inviteStore';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const room = $derived(data.room);
  const slug = $derived(room?.id ?? '');
  const currentUser = $derived(ndk.$currentUser);
  const isAdmin = $derived(
    !!currentUser && (room?.adminPubkeys ?? []).includes(currentUser.pubkey)
  );
  const isOpen = $derived(room?.access === 'open');
  const isFresh = $derived(data.fresh);

  let invites = $state<InviteRecord[]>([]);
  let loaded = $state(false);

  let mode = $state<'single' | 'multiple'>('single');
  let singleLabel = $state('');
  let batchCount = $state(5);
  let mintPending = $state(false);
  let mintError = $state('');
  let autoMintAttempted = $state(false);

  let copiedCode = $state('');
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const origin = $derived(browser ? window.location.origin : '');
  function urlFor(code: string): string {
    return `${origin}/r/${slug}/join/${code}`;
  }

  $effect(() => {
    if (!browser) return;
    if (!currentUser || !slug) return;
    invites = listInvites(currentUser.pubkey, slug);
    loaded = true;
  });

  // Auto-mint one shareable code on first visit to a freshly-created closed
  // room if the creator has no invites yet. Gives them something to paste
  // without a second click.
  $effect(() => {
    if (!browser) return;
    if (!loaded) return;
    if (autoMintAttempted) return;
    if (!isFresh) return;
    if (!isAdmin) return;
    if (isOpen) return;
    if (invites.length > 0) return;
    autoMintAttempted = true;
    void mint(1, '');
  });

  async function mint(count: number, label: string) {
    if (!currentUser) {
      mintError = 'Sign in first.';
      return;
    }
    if (!isAdmin) {
      mintError = 'Only admins can create invites.';
      return;
    }
    try {
      mintPending = true;
      mintError = '';
      await ensureClientNdk();
      const results = await createInviteCodes(ndk, slug, { count });
      const codes = results.flatMap((r) => r.codes);
      if (count === 1) {
        recordInvite(currentUser.pubkey, slug, { code: codes[0], label });
      } else {
        recordInvites(
          currentUser.pubkey,
          slug,
          codes.map((code) => ({ code, label }))
        );
      }
      invites = listInvites(currentUser.pubkey, slug);
      singleLabel = '';
    } catch (error) {
      mintError = error instanceof Error ? error.message : 'Could not create invites.';
    } finally {
      mintPending = false;
    }
  }

  async function submitCreate(event: SubmitEvent) {
    event.preventDefault();
    if (mode === 'single') {
      await mint(1, singleLabel);
    } else {
      const n = Math.max(1, Math.min(batchCount, MAX_CODES_PER_INVITE_EVENT));
      await mint(n, '');
    }
  }

  async function copyLink(code: string) {
    try {
      await navigator.clipboard.writeText(urlFor(code));
      copiedCode = code;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copiedCode = '';
      }, 1800);
    } catch {
      // fall through silently
    }
  }

  function handleDelete(code: string) {
    if (!currentUser) return;
    deleteInvite(currentUser.pubkey, slug, code);
    invites = listInvites(currentUser.pubkey, slug);
  }

  function formatDate(ts: number): string {
    const ms = ts * 1000;
    const d = new Date(ms);
    return d.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric'
    });
  }
</script>

<svelte:head>
  <title>Invite — {room?.name ?? slug}</title>
</svelte:head>

{#if !room}
  <div class="empty-state">
    <h1>Room not found</h1>
    <a href="/rooms" class="btn">Back to your rooms</a>
  </div>
{:else if !currentUser}
  <div class="empty-state">
    <h1>Sign in to manage invites</h1>
    <a href="/onboarding" class="btn">Sign in</a>
  </div>
{:else if !isAdmin}
  <div class="empty-state">
    <h1>Only admins can manage invites</h1>
    <a href="/r/{slug}" class="btn">Back to the room</a>
  </div>
{:else}
  <section class="invite-page">
    <header class="invite-head">
      <a class="back-link" href="/r/{slug}">← {room.name}</a>
      {#if isFresh}
        <h1>Your room is live.</h1>
        <p class="lead">
          {#if isOpen}
            It's open — anyone with the link below can join and read along.
          {:else}
            Now invite the people you want in the room.
          {/if}
        </p>
      {:else}
        <h1>Invites</h1>
        <p class="lead">
          {#if isOpen}
            This room is open — anyone with the room link can join.
          {:else}
            Create invite links for the people you want in the room.
          {/if}
        </p>
      {/if}
    </header>

    {#if isOpen}
      <section class="open-room-card">
        <div class="invite-label">Shareable link</div>
        <div class="invite-link-row">
          <code class="invite-link">{origin}/r/{slug}</code>
          <button
            type="button"
            class="btn btn-ghost btn-sm"
            onclick={async () => {
              try {
                await navigator.clipboard.writeText(`${origin}/r/${slug}`);
                copiedCode = 'open';
                if (copyTimer) clearTimeout(copyTimer);
                copyTimer = setTimeout(() => {
                  copiedCode = '';
                }, 1800);
              } catch {}
            }}
          >
            {copiedCode === 'open' ? 'Copied' : 'Copy'}
          </button>
        </div>
        <p class="hint">
          Anyone with this link can read and join without approval. Change this
          in <a href="/r/{slug}/settings">room settings</a>.
        </p>
      </section>
    {:else}
      <section class="create-card">
        <form onsubmit={submitCreate}>
          <div class="mode-switch" role="tablist">
            <button
              type="button"
              role="tab"
              class="mode-btn"
              class:active={mode === 'single'}
              aria-selected={mode === 'single'}
              onclick={() => (mode = 'single')}
            >
              For a specific person
            </button>
            <button
              type="button"
              role="tab"
              class="mode-btn"
              class:active={mode === 'multiple'}
              aria-selected={mode === 'multiple'}
              onclick={() => (mode = 'multiple')}
            >
              Multiple at once
            </button>
          </div>

          {#if mode === 'single'}
            <div class="create-row">
              <input
                class="field-input"
                type="text"
                bind:value={singleLabel}
                placeholder="Who's it for? (e.g. Maria)"
                maxlength="60"
                autocomplete="off"
              />
              <button
                type="submit"
                class="btn btn-primary"
                disabled={mintPending}
              >
                {mintPending ? 'Creating…' : 'Create invite'}
              </button>
            </div>
            <p class="hint">
              The label is just for your own list. Each link works once.
            </p>
          {:else}
            <div class="create-row">
              <input
                class="field-input field-count"
                type="number"
                min="1"
                max={MAX_CODES_PER_INVITE_EVENT}
                bind:value={batchCount}
              />
              <span class="count-label">links</span>
              <button
                type="submit"
                class="btn btn-primary"
                disabled={mintPending}
              >
                {mintPending ? 'Creating…' : 'Create'}
              </button>
            </div>
            <p class="hint">
              Up to {MAX_CODES_PER_INVITE_EVENT} at a time. Each link works once.
            </p>
          {/if}

          {#if mintError}
            <p class="error-banner">{mintError}</p>
          {/if}
        </form>
      </section>

      <section class="list-card">
        {#if !loaded}
          <p class="muted">Loading…</p>
        {:else if invites.length === 0}
          <p class="muted">No invite links yet. Create one above.</p>
        {:else}
          <div class="list-head">
            <span class="list-count">
              {invites.length} invite{invites.length === 1 ? '' : 's'}
            </span>
          </div>
          <ul class="invite-list">
            {#each invites as invite (invite.code)}
              <li class="invite-row">
                <div class="row-body">
                  <div class="row-top">
                    {#if invite.label}
                      <strong class="row-label">{invite.label}</strong>
                    {:else}
                      <span class="row-label row-label-muted">Shareable link</span>
                    {/if}
                    <span class="row-date">· {formatDate(invite.createdAt)}</span>
                  </div>
                  <code class="invite-link">{urlFor(invite.code)}</code>
                </div>
                <div class="row-actions">
                  <button
                    type="button"
                    class="btn btn-ghost btn-sm"
                    onclick={() => copyLink(invite.code)}
                  >
                    {copiedCode === invite.code ? 'Copied' : 'Copy'}
                  </button>
                  <button
                    type="button"
                    class="btn btn-ghost btn-sm btn-quiet"
                    onclick={() => handleDelete(invite.code)}
                    title="Remove from your list"
                    aria-label="Remove from your list"
                  >
                    Remove
                  </button>
                </div>
              </li>
            {/each}
          </ul>
          <p class="hint">
            Removing a link only hides it from your list. The code stays valid
            on the relay until it's used.
          </p>
        {/if}
      </section>
    {/if}

    <footer class="invite-foot">
      <a class="btn btn-ghost" href="/r/{slug}">Enter the room →</a>
    </footer>
  </section>
{/if}

<style>
  .invite-page {
    display: grid;
    gap: 2rem;
    max-width: 42rem;
    margin: 0 auto;
    padding: 2.5rem 0 4rem;
  }

  .invite-head {
    display: grid;
    gap: 0.65rem;
  }

  .back-link {
    color: var(--accent);
    font-family: var(--font-sans);
    font-size: 0.85rem;
    font-weight: 500;
    text-decoration: none;
  }

  .back-link:hover {
    text-decoration: underline;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.8rem, 4vw, 2.6rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .lead {
    margin: 0;
    color: var(--muted);
    font-size: 1rem;
    line-height: 1.55;
  }

  .create-card,
  .list-card,
  .open-room-card {
    display: grid;
    gap: 0.85rem;
    padding: 1.25rem 1.35rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .mode-switch {
    display: inline-flex;
    gap: 0;
    border: 1px solid var(--color-base-300);
    border-radius: 999px;
    padding: 3px;
    background: var(--surface-soft);
    justify-self: start;
  }

  .mode-btn {
    padding: 0.45rem 0.9rem;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-sans);
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
  }

  .mode-btn.active {
    background: var(--text-strong);
    color: var(--surface);
  }

  .create-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.6rem;
    align-items: center;
  }

  .create-row:has(.field-count) {
    grid-template-columns: 6rem auto 1fr;
  }

  .field-input {
    width: 100%;
    padding: 0.625rem 0.8rem;
    border: 1px solid var(--color-base-300);
    border-radius: 0.65rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-size: 0.9rem;
    font-family: inherit;
    outline: none;
    transition: border-color 120ms ease;
  }

  .field-input:focus {
    border-color: var(--accent);
  }

  .field-count {
    text-align: center;
  }

  .count-label {
    color: var(--muted);
    font-size: 0.9rem;
    justify-self: start;
  }

  .create-row:has(.field-count) .btn {
    grid-column: 3;
    justify-self: end;
  }

  .hint,
  .muted {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
    line-height: 1.55;
  }

  .hint a {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .invite-label {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .invite-link-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.6rem;
    align-items: center;
  }

  .invite-link {
    display: block;
    padding: 0.55rem 0.7rem;
    border: 1px dashed var(--color-base-300);
    border-radius: 0.5rem;
    background: var(--surface-soft);
    color: var(--text-strong);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .list-count {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .invite-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.1rem;
  }

  .invite-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.9rem;
    align-items: center;
    padding: 0.75rem 0;
    border-top: 1px dotted var(--color-base-300);
  }

  .invite-list .invite-row:first-child {
    border-top: none;
  }

  .row-body {
    display: grid;
    gap: 0.35rem;
    min-width: 0;
  }

  .row-top {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .row-label {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1rem;
    font-weight: 500;
  }

  .row-label-muted {
    color: var(--muted);
    font-weight: 400;
    font-style: italic;
  }

  .row-date {
    color: var(--muted);
    font-size: 0.82rem;
  }

  .row-actions {
    display: flex;
    gap: 0.35rem;
  }

  .btn-sm {
    padding: 0.35rem 0.7rem;
    font-size: 0.78rem;
  }

  .btn-quiet {
    color: var(--muted);
  }

  .error-banner {
    margin: 0;
    padding: 0.7rem 0.85rem;
    border-radius: 0.65rem;
    background: var(--pale-red);
    color: var(--pale-red-text);
    font-size: 0.85rem;
  }

  .invite-foot {
    display: flex;
    justify-content: flex-end;
    padding-top: 0.5rem;
  }

  .empty-state {
    display: grid;
    gap: 1rem;
    justify-items: center;
    padding: 5rem 0;
    text-align: center;
  }

  .empty-state h1 {
    font-size: 1.6rem;
  }
</style>
