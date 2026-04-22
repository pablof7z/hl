<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import {
    createRoom,
    slugifyRoomId,
    isValidRoomId,
    type RoomAccess,
    type RoomVisibility
  } from '$lib/ndk/groups';

  type Preset = 'invite' | 'open' | 'members';

  const PRESET_MAP: Record<Preset, { access: RoomAccess; visibility: RoomVisibility }> = {
    invite: { access: 'closed', visibility: 'public' },
    open: { access: 'open', visibility: 'public' },
    members: { access: 'closed', visibility: 'private' }
  };

  let step = $state<1 | 2 | 3>(1);

  let name = $state('');
  let roomId = $state('');
  let roomIdEdited = $state(false);
  let preset = $state<Preset>('invite');
  let about = $state('');
  let picture = $state('');

  let publishing = $state(false);
  let errorMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const slug = $derived(slugifyRoomId(roomId));
  const slugIsValid = $derived(isValidRoomId(slug));
  const step1Complete = $derived(name.trim().length > 0 && slugIsValid);

  $effect(() => {
    if (!roomIdEdited) {
      roomId = slugifyRoomId(name);
    }
  });

  function handleSlugInput(event: Event) {
    roomIdEdited = true;
    roomId = slugifyRoomId((event.currentTarget as HTMLInputElement).value);
  }

  function goNext() {
    errorMessage = '';
    if (step === 1) {
      if (!step1Complete) {
        errorMessage = 'Give the room a name first.';
        return;
      }
      step = 2;
      return;
    }
    if (step === 2) {
      step = 3;
      return;
    }
  }

  function goBack() {
    errorMessage = '';
    if (step === 2) step = 1;
    else if (step === 3) step = 2;
  }

  async function publishRoom() {
    if (!currentUser) {
      errorMessage = 'Sign in before creating a room.';
      return;
    }
    if (isReadOnly) {
      errorMessage = 'Read-only sessions cannot create rooms.';
      return;
    }
    if (!step1Complete) {
      errorMessage = 'The room needs a name and a valid URL.';
      step = 1;
      return;
    }

    try {
      publishing = true;
      errorMessage = '';
      await ensureClientNdk();
      const { access, visibility } = PRESET_MAP[preset];
      const result = await createRoom(ndk, {
        id: slug,
        name,
        about,
        picture,
        access,
        visibility
      });
      await goto(`/r/${result.id}/invite?fresh=1`, { invalidateAll: true });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not create the room.';
    } finally {
      publishing = false;
    }
  }
</script>

<svelte:head>
  <title>New room — Highlighter</title>
</svelte:head>

<section class="grid gap-8 max-w-[38rem] mx-auto py-12 pb-16">
  <header class="flex items-center justify-between">
    <div class="font-mono text-[0.72rem] tracking-[0.2em] uppercase text-base-content/50">0{step} / 03</div>
    <div class="inline-flex gap-[0.4rem]" role="tablist" aria-label="Wizard progress">
      {#each [1, 2, 3] as s}
        <span
          class="w-[6px] h-[6px] rounded-full {step === s ? 'bg-primary' : step > s ? 'bg-base-content' : 'bg-base-300'}"
          aria-hidden="true"
        ></span>
      {/each}
    </div>
  </header>

  {#if step === 1}
    <div class="grid gap-6 min-h-[18rem]">
      <h1 class="m-0 text-base-content font-serif text-[clamp(2rem,5vw,2.8rem)] leading-[1.05] tracking-[-0.03em]">What do you want to call it?</h1>

      <label class="grid gap-[0.4rem]">
        <span class="font-mono text-[0.72rem] tracking-[0.18em] uppercase text-base-content/50">Room name</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="w-full py-[0.9rem] border-0 border-b border-base-300 bg-transparent text-base-content font-serif text-[clamp(1.5rem,3.5vw,2rem)] leading-[1.15] tracking-[-0.02em] outline-none transition-[border-color] duration-[120ms] ease-in-out placeholder:text-base-content/50 focus:border-primary"
          type="text"
          bind:value={name}
          placeholder="Signal over noise"
          maxlength="80"
          autocomplete="off"
          autofocus
        />
      </label>

      <div class="grid grid-cols-[auto_1fr] items-baseline py-[0.65rem] border-b border-dotted border-base-300">
        <span class="text-base-content/50 font-mono text-[0.82rem]">highlighter.com/r/</span>
        <input
          class="min-w-0 p-0 border-0 bg-transparent text-base-content font-mono text-[0.82rem] outline-none"
          value={roomId}
          oninput={handleSlugInput}
          placeholder="signal-over-noise"
          maxlength="48"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      {#if roomId && !slugIsValid}
        <p class="m-0 text-[0.8rem] leading-[1.5] text-error">Use 3–48 lowercase letters, numbers, and hyphens.</p>
      {:else}
        <p class="m-0 text-[0.8rem] leading-[1.5] text-base-content/50">Lowercase letters, numbers, and hyphens. The room's address.</p>
      {/if}
    </div>
  {/if}

  {#if step === 2}
    <div class="grid gap-6 min-h-[18rem]">
      <h1 class="m-0 text-base-content font-serif text-[clamp(2rem,5vw,2.8rem)] leading-[1.05] tracking-[-0.03em]">Who can read and join?</h1>

      <div class="grid gap-3">
        <label class="grid grid-cols-[auto_1fr] gap-4 items-start px-5 py-[1.1rem] border border-base-300 rounded-2xl bg-base-100 cursor-pointer transition-[border-color,background] duration-[120ms] ease-in-out hover:border-base-content {preset === 'invite' ? 'border-primary bg-primary/[0.04]' : ''}">
          <input class="mt-[0.35rem]" type="radio" bind:group={preset} value="invite" />
          <div class="grid gap-1">
            <strong class="text-base-content font-serif text-[1.15rem] font-medium tracking-[-0.01em]">By invitation</strong>
            <p class="m-0 text-base-content/50 text-[0.9rem] leading-[1.55]">Only people you invite can join. Anyone with the link can read along.</p>
          </div>
        </label>

        <label class="grid grid-cols-[auto_1fr] gap-4 items-start px-5 py-[1.1rem] border border-base-300 rounded-2xl bg-base-100 cursor-pointer transition-[border-color,background] duration-[120ms] ease-in-out hover:border-base-content {preset === 'open' ? 'border-primary bg-primary/[0.04]' : ''}">
          <input class="mt-[0.35rem]" type="radio" bind:group={preset} value="open" />
          <div class="grid gap-1">
            <strong class="text-base-content font-serif text-[1.15rem] font-medium tracking-[-0.01em]">Open to anyone</strong>
            <p class="m-0 text-base-content/50 text-[0.9rem] leading-[1.55]">Anyone can join. Anyone can read.</p>
          </div>
        </label>

        <label class="grid grid-cols-[auto_1fr] gap-4 items-start px-5 py-[1.1rem] border border-base-300 rounded-2xl bg-base-100 cursor-pointer transition-[border-color,background] duration-[120ms] ease-in-out hover:border-base-content {preset === 'members' ? 'border-primary bg-primary/[0.04]' : ''}">
          <input class="mt-[0.35rem]" type="radio" bind:group={preset} value="members" />
          <div class="grid gap-1">
            <strong class="text-base-content font-serif text-[1.15rem] font-medium tracking-[-0.01em]">Members only</strong>
            <p class="m-0 text-base-content/50 text-[0.9rem] leading-[1.55]">Only members can join. Only members can see what's inside.</p>
          </div>
        </label>
      </div>

      <p class="m-0 text-base-content/50 text-[0.9rem] leading-[1.55]">You can change this later in room settings.</p>
    </div>
  {/if}

  {#if step === 3}
    <div class="grid gap-6 min-h-[18rem]">
      <h1 class="m-0 text-base-content font-serif text-[clamp(2rem,5vw,2.8rem)] leading-[1.05] tracking-[-0.03em]">Describe it.</h1>

      <fieldset class="grid gap-2 border-none p-0 m-0">
        <legend class="font-mono text-[0.72rem] tracking-[0.18em] uppercase text-base-content/50">What's this room about?</legend>
        <textarea
          class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-100 text-base-content text-[0.9rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] ease-in-out resize-y placeholder:text-base-content/50 focus:border-primary"
          bind:value={about}
          rows="4"
          maxlength="280"
          placeholder="Essays, books, and podcasts we keep coming back to."
        ></textarea>
      </fieldset>

      <fieldset class="grid gap-2 border-none p-0 m-0">
        <legend class="font-mono text-[0.72rem] tracking-[0.18em] uppercase text-base-content/50">Cover image URL</legend>
        <input
          class="w-full px-3 py-[0.625rem] border border-base-300 rounded-xl bg-base-100 text-base-content text-[0.9rem] font-[inherit] outline-none transition-[border-color] duration-[120ms] ease-in-out placeholder:text-base-content/50 focus:border-primary"
          bind:value={picture}
          placeholder="https://..."
          inputmode="url"
          autocomplete="off"
        />
      </fieldset>

      <p class="m-0 text-base-content/50 text-[0.9rem] leading-[1.55]">Both are optional — you can add them after the room is live.</p>
    </div>
  {/if}

  {#if errorMessage}
    <p class="m-0 px-4 py-[0.85rem] rounded-[0.9rem] text-[0.9rem] leading-[1.5] bg-error/10 text-error">{errorMessage}</p>
  {/if}

  {#if isReadOnly}
    <p class="m-0 px-4 py-[0.85rem] rounded-[0.9rem] text-[0.9rem] leading-[1.5] bg-warning/10 text-warning">
      This signer is read-only. Switch to a writable signer to create a room.
    </p>
  {/if}

  <footer class="flex justify-between items-center gap-4 pt-4 border-t border-base-300">
    {#if step > 1}
      <button type="button" class="btn btn-ghost" onclick={goBack} disabled={publishing}>
        Back
      </button>
    {:else}
      <span></span>
    {/if}

    {#if step < 3}
      <button
        type="button"
        class="btn btn-primary"
        onclick={goNext}
        disabled={step === 1 && !step1Complete}
      >
        Continue
      </button>
    {:else}
      <button
        type="button"
        class="btn btn-primary"
        onclick={publishRoom}
        disabled={publishing || isReadOnly || !step1Complete}
      >
        {publishing ? 'Creating…' : 'Create the room'}
      </button>
    {/if}
  </footer>
</section>
