<script lang="ts">
  import { goto } from '$app/navigation';
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import DiscussionPanel from '$lib/features/discussions/DiscussionPanel.svelte';
  import { relativeTime } from '$lib/utils/time';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const discussion = $derived(data.discussion);
  const room = $derived(data.room);

  const rootContext = $derived(
    discussion
      ? {
          type: 'share-thread' as const,
          shareThreadEventId: discussion.eventId
        }
      : undefined
  );

  const when = $derived(discussion ? relativeTime(discussion.createdAt) : '');

  function handleBack() {
    void goto(room ? `/r/${room.id}` : '/rooms');
  }
</script>

<svelte:head>
  <title>{discussion?.title ?? 'Discussion'} · Room</title>
</svelte:head>

{#if !room || !discussion}
  <div class="py-20 text-center flex flex-col gap-4 items-center">
    <h1 class="font-serif text-[34px] font-normal text-base-content m-0">Discussion not found</h1>
    <p class="text-base-content/80 text-[15px] max-w-[44ch] m-0">The post isn't on the relays we queried, or it has been removed.</p>
    <a href={room ? `/r/${room.id}` : '/rooms'} class="inline-flex items-center px-5 py-2.5 bg-base-content text-base-100 font-sans text-[13px] font-medium no-underline rounded hover:bg-primary transition-colors duration-200">Back to the room</a>
  </div>
{:else}
  <article class="max-w-[720px] mx-auto py-7 pb-24 flex flex-col gap-6">
    <button type="button" class="self-start border-0 bg-transparent p-0 py-1 font-sans text-[12px] tracking-[0.02em] text-base-content/50 cursor-pointer transition-colors duration-150 hover:text-primary" onclick={handleBack}>
      ← {room.name ?? room.id}
    </button>

    <header class="flex flex-col gap-[14px] pb-2 border-b border-base-300">
      <div class="flex items-center gap-[10px]">
        <User.Root {ndk} pubkey={discussion.pubkey}>
          <span class="w-7 h-7 rounded-full overflow-hidden shrink-0">
            <User.Avatar class="w-full h-full" />
          </span>
          <span class="font-sans text-[13px] text-base-content/50 italic">
            started by
            <b class="text-base-content font-semibold not-italic"><User.Name field="displayName" /></b>
            <span class="font-mono text-[11px] text-base-content/50 not-italic ml-1 tracking-[0.04em]">· {when}</span>
          </span>
        </User.Root>
      </div>
      <h1 class="font-serif text-[34px] leading-[1.15] tracking-[-0.015em] text-base-content m-0 font-normal max-sm:text-[26px]">{discussion.title}</h1>
    </header>

    {#if discussion.attachment}
      <a
        class="grid grid-cols-[84px_minmax(0,1fr)] gap-[14px] p-[14px] border border-base-300 rounded bg-base-100 no-underline text-inherit transition-[border-color,background] duration-150 hover:border-primary max-sm:grid-cols-1"
        href={discussion.attachment.url || '#'}
        target={discussion.attachment.url ? '_blank' : undefined}
        rel={discussion.attachment.url ? 'noreferrer' : undefined}
      >
        {#if discussion.attachment.image}
          <img src={discussion.attachment.image} alt="" loading="lazy" class="w-full h-full aspect-[4/5] object-cover rounded-[6px] max-sm:aspect-video" />
        {:else}
          <div class="flex items-center justify-center aspect-[4/5] bg-base-content text-base-100 font-serif text-[22px] rounded-[6px] max-sm:aspect-video" aria-hidden="true">
            {(discussion.attachment.title || discussion.attachment.source).slice(0, 2).toUpperCase()}
          </div>
        {/if}
        <div class="flex flex-col gap-1 min-w-0">
          <span class="font-mono text-[10px] uppercase tracking-[0.08em] text-primary font-semibold">{discussion.attachment.source}</span>
          <strong class="font-sans text-[14.5px] leading-[1.3] text-base-content font-semibold overflow-hidden text-ellipsis line-clamp-2">{discussion.attachment.title || discussion.attachment.url}</strong>
          {#if discussion.attachment.author}
            <span class="text-[12.5px] text-base-content/50 italic">{discussion.attachment.author}</span>
          {/if}
        </div>
      </a>
    {/if}

    {#if discussion.body}
      <div class="font-sans text-[15.5px] leading-[1.62] text-base-content/80 flex flex-col gap-[14px] max-sm:text-[14.5px]">
        {#each discussion.body.split(/\n{2,}/) as para, i (i)}
          <p class="m-0 whitespace-pre-wrap">{para}</p>
        {/each}
      </div>
    {/if}

    <section class="mt-4 pt-7 border-t border-base-300">
      <DiscussionPanel
        groupId={room.id}
        rootContext={rootContext!}
        showHeader={true}
      />
    </section>
  </article>
{/if}
