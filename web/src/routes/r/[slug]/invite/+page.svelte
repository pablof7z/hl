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
  <div class="grid gap-4 justify-items-center py-20 text-center">
    <h1 class="text-[1.6rem]">Room not found</h1>
    <a href="/rooms" class="btn">Back to your rooms</a>
  </div>
{:else if !currentUser}
  <div class="grid gap-4 justify-items-center py-20 text-center">
    <h1 class="text-[1.6rem]">Sign in to manage invites</h1>
    <a href="/onboarding" class="btn">Sign in</a>
  </div>
{:else if !isAdmin}
  <div class="grid gap-4 justify-items-center py-20 text-center">
    <h1 class="text-[1.6rem]">Only admins can manage invites</h1>
    <a href="/r/{slug}" class="btn">Back to the room</a>
  </div>
{:else}
  <section class="grid gap-8 max-w-[42rem] mx-auto pt-10 pb-16">
    <header class="grid gap-[0.65rem]">
      <a class="text-primary text-[0.85rem] font-medium no-underline hover:underline" href="/r/{slug}">← {room.name}</a>
      {#if isFresh}
        <h1 class="m-0 text-base-content font-serif text-[clamp(1.8rem,4vw,2.6rem)] leading-[1.05] tracking-[-0.03em]">Your room is live.</h1>
        <p class="m-0 text-base-content/50 text-base leading-[1.55]">
          {#if isOpen}
            It's open — anyone with the link below can join and read along.
          {:else}
            Now invite the people you want in the room.
          {/if}
        </p>
      {:else}
        <h1 class="m-0 text-base-content font-serif text-[clamp(1.8rem,4vw,2.6rem)] leading-[1.05] tracking-[-0.03em]">Invites</h1>
        <p class="m-0 text-base-content/50 text-base leading-[1.55]">
          {#if isOpen}
            This room is open — anyone with the room link can join.
          {:else}
            Create invite links for the people you want in the room.
          {/if}
        </p>
      {/if}
    </header>

    {#if isOpen}
      <section class="grid gap-[0.85rem] p-[1.25rem_1.35rem] border border-base-300 rounded-[1.1rem] bg-base-100">
        <div class="font-mono text-[0.72rem] tracking-[0.18em] uppercase text-base-content/50">Shareable link</div>
        <div class="grid [grid-template-columns:1fr_auto] gap-[0.6rem] items-center">
          <code class="block px-[0.7rem] py-[0.55rem] border border-dashed border-base-300 rounded-lg bg-base-200 text-base-content font-mono text-[0.8rem] whitespace-nowrap overflow-hidden text-ellipsis">{origin}/r/{slug}</code>
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
        <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">
          Anyone with this link can read and join without approval. Change this
          in <a href="/r/{slug}/settings" class="text-primary underline underline-offset-[2px]">room settings</a>.
        </p>
      </section>
    {:else}
      <section class="grid gap-[0.85rem] p-[1.25rem_1.35rem] border border-base-300 rounded-[1.1rem] bg-base-100">
        <form onsubmit={submitCreate}>
          <div class="inline-flex gap-0 border border-base-300 rounded-full p-[3px] bg-base-200 justify-self-start" role="tablist">
            <button
              type="button"
              role="tab"
              class="px-[0.9rem] py-[0.45rem] border-0 rounded-full text-[0.82rem] font-medium cursor-pointer transition-colors {mode === 'single' ? 'bg-base-content text-base-100' : 'bg-transparent text-base-content/50'}"
              aria-selected={mode === 'single'}
              onclick={() => (mode = 'single')}
            >
              For a specific person
            </button>
            <button
              type="button"
              role="tab"
              class="px-[0.9rem] py-[0.45rem] border-0 rounded-full text-[0.82rem] font-medium cursor-pointer transition-colors {mode === 'multiple' ? 'bg-base-content text-base-100' : 'bg-transparent text-base-content/50'}"
              aria-selected={mode === 'multiple'}
              onclick={() => (mode = 'multiple')}
            >
              Multiple at once
            </button>
          </div>

          {#if mode === 'single'}
            <div class="grid [grid-template-columns:1fr_auto] gap-[0.6rem] items-center mt-[0.85rem]">
              <input
                class="w-full px-[0.8rem] py-[0.625rem] border border-base-300 rounded-[0.65rem] bg-base-200 text-base-content text-[0.9rem] outline-none transition-colors duration-[120ms] ease focus:border-primary"
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
            <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55] mt-[0.85rem]">
              The label is just for your own list. Each link works once.
            </p>
          {:else}
            <div class="grid [grid-template-columns:6rem_auto_1fr] gap-[0.6rem] items-center mt-[0.85rem]">
              <input
                class="w-full px-[0.8rem] py-[0.625rem] border border-base-300 rounded-[0.65rem] bg-base-200 text-base-content text-[0.9rem] outline-none transition-colors duration-[120ms] ease focus:border-primary text-center"
                type="number"
                min="1"
                max={MAX_CODES_PER_INVITE_EVENT}
                bind:value={batchCount}
              />
              <span class="text-base-content/50 text-[0.9rem] justify-self-start">links</span>
              <button
                type="submit"
                class="btn btn-primary justify-self-end"
                disabled={mintPending}
              >
                {mintPending ? 'Creating…' : 'Create'}
              </button>
            </div>
            <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55] mt-[0.85rem]">
              Up to {MAX_CODES_PER_INVITE_EVENT} at a time. Each link works once.
            </p>
          {/if}

          {#if mintError}
            <p class="m-0 mt-[0.85rem] px-[0.85rem] py-[0.7rem] rounded-[0.65rem] bg-error/10 text-error text-[0.85rem]">{mintError}</p>
          {/if}
        </form>
      </section>

      <section class="grid gap-[0.85rem] p-[1.25rem_1.35rem] border border-base-300 rounded-[1.1rem] bg-base-100">
        {#if !loaded}
          <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">Loading…</p>
        {:else if invites.length === 0}
          <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">No invite links yet. Create one above.</p>
        {:else}
          <div class="flex justify-between items-baseline">
            <span class="font-mono text-[0.72rem] tracking-[0.18em] uppercase text-base-content/50">
              {invites.length} invite{invites.length === 1 ? '' : 's'}
            </span>
          </div>
          <ul class="list-none m-0 p-0 grid gap-[0.1rem]">
            {#each invites as invite (invite.code)}
              <li class="grid [grid-template-columns:1fr_auto] gap-[0.9rem] items-center py-3 border-t border-dotted border-base-300 first:border-t-0">
                <div class="grid gap-[0.35rem] min-w-0">
                  <div class="flex items-baseline gap-[0.35rem] flex-wrap">
                    {#if invite.label}
                      <strong class="text-base-content font-serif text-base font-medium">{invite.label}</strong>
                    {:else}
                      <span class="text-base-content/50 font-serif text-base font-normal italic">Shareable link</span>
                    {/if}
                    <span class="text-base-content/50 text-[0.82rem]">· {formatDate(invite.createdAt)}</span>
                  </div>
                  <code class="block px-[0.7rem] py-[0.55rem] border border-dashed border-base-300 rounded-lg bg-base-200 text-base-content font-mono text-[0.8rem] whitespace-nowrap overflow-hidden text-ellipsis">{urlFor(invite.code)}</code>
                </div>
                <div class="flex gap-[0.35rem]">
                  <button
                    type="button"
                    class="btn btn-ghost btn-sm"
                    onclick={() => copyLink(invite.code)}
                  >
                    {copiedCode === invite.code ? 'Copied' : 'Copy'}
                  </button>
                  <button
                    type="button"
                    class="btn btn-ghost btn-sm text-base-content/50"
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
          <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.55]">
            Removing a link only hides it from your list. The code stays valid
            on the relay until it's used.
          </p>
        {/if}
      </section>
    {/if}

    <footer class="flex justify-end pt-2">
      <a class="btn btn-ghost" href="/r/{slug}">Enter the room →</a>
    </footer>
  </section>
{/if}
