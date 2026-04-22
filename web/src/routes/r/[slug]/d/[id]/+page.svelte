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
  <div class="missing">
    <h1>Discussion not found</h1>
    <p>The post isn't on the relays we queried, or it has been removed.</p>
    <a href={room ? `/r/${room.id}` : '/rooms'} class="btn">Back to the room</a>
  </div>
{:else}
  <article class="discussion">
    <button type="button" class="crumb" onclick={handleBack}>
      ← {room.name ?? room.id}
    </button>

    <header class="d-head">
      <div class="d-meta">
        <User.Root {ndk} pubkey={discussion.pubkey}>
          <span class="d-avatar">
            <User.Avatar />
          </span>
          <span class="d-by">
            started by
            <b><User.Name field="displayName" /></b>
            <span class="d-time">· {when}</span>
          </span>
        </User.Root>
      </div>
      <h1>{discussion.title}</h1>
    </header>

    {#if discussion.attachment}
      <a
        class="d-attachment"
        href={discussion.attachment.url || '#'}
        target={discussion.attachment.url ? '_blank' : undefined}
        rel={discussion.attachment.url ? 'noreferrer' : undefined}
      >
        {#if discussion.attachment.image}
          <img src={discussion.attachment.image} alt="" loading="lazy" />
        {:else}
          <div class="d-attachment-placeholder" aria-hidden="true">
            {(discussion.attachment.title || discussion.attachment.source).slice(0, 2).toUpperCase()}
          </div>
        {/if}
        <div class="d-attachment-copy">
          <span class="d-attachment-kind">{discussion.attachment.source}</span>
          <strong>{discussion.attachment.title || discussion.attachment.url}</strong>
          {#if discussion.attachment.author}
            <span class="d-attachment-author">{discussion.attachment.author}</span>
          {/if}
        </div>
      </a>
    {/if}

    {#if discussion.body}
      <div class="d-body">
        {#each discussion.body.split(/\n{2,}/) as para, i (i)}
          <p>{para}</p>
        {/each}
      </div>
    {/if}

    <section class="d-replies">
      <DiscussionPanel
        groupId={room.id}
        rootContext={rootContext!}
        showHeader={true}
      />
    </section>
  </article>
{/if}

<style>
  .missing {
    padding: 80px 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: center;
  }

  .missing h1 {
    font-family: var(--font-serif);
    font-size: 34px;
    font-weight: 400;
    color: var(--ink);
    margin: 0;
  }

  .missing p {
    color: var(--ink-soft);
    font-size: 15px;
    max-width: 44ch;
    margin: 0;
  }

  .discussion {
    max-width: 720px;
    margin: 0 auto;
    padding: 28px 0 96px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .crumb {
    align-self: flex-start;
    border: 0;
    background: transparent;
    padding: 4px 0;
    font-family: var(--font-sans);
    font-size: 12px;
    letter-spacing: 0.02em;
    color: var(--ink-fade);
    cursor: pointer;
    transition: color 150ms ease;
  }

  .crumb:hover { color: var(--brand-accent); }

  .d-head {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule);
  }

  .d-meta {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .d-meta :global(.d-avatar) {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    overflow: hidden;
    flex-shrink: 0;
  }

  .d-meta :global(.d-avatar img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .d-by {
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ink-fade);
    font-style: italic;
  }

  .d-by :global(b) {
    color: var(--ink);
    font-weight: 600;
    font-style: normal;
  }

  .d-time {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    font-style: normal;
    margin-left: 4px;
    letter-spacing: 0.04em;
  }

  .d-head h1 {
    font-family: var(--font-serif);
    font-size: 34px;
    line-height: 1.15;
    letter-spacing: -0.015em;
    color: var(--ink);
    margin: 0;
    font-weight: 400;
  }

  .d-attachment {
    display: grid;
    grid-template-columns: 84px minmax(0, 1fr);
    gap: 14px;
    padding: 14px;
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    background: var(--surface);
    text-decoration: none;
    color: inherit;
    transition: border-color 150ms ease, background 150ms ease;
  }

  .d-attachment:hover {
    border-color: var(--brand-accent);
    background: color-mix(in srgb, var(--brand-accent) 4%, var(--surface));
  }

  .d-attachment img {
    width: 100%;
    height: 100%;
    aspect-ratio: 4 / 5;
    object-fit: cover;
    border-radius: 6px;
  }

  .d-attachment-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 4 / 5;
    background: var(--ink);
    color: var(--surface);
    font-family: var(--font-serif);
    font-size: 22px;
    border-radius: 6px;
  }

  .d-attachment-copy {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .d-attachment-kind {
    font-family: var(--font-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--brand-accent);
    font-weight: 600;
  }

  .d-attachment-copy strong {
    font-family: var(--font-sans);
    font-size: 14.5px;
    line-height: 1.3;
    color: var(--ink);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .d-attachment-author {
    font-size: 12.5px;
    color: var(--ink-fade);
    font-style: italic;
  }

  .d-body {
    font-family: var(--font-sans);
    font-size: 15.5px;
    line-height: 1.62;
    color: var(--ink-soft);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .d-body p {
    margin: 0;
    white-space: pre-wrap;
  }

  .d-replies {
    margin-top: 16px;
    padding-top: 28px;
    border-top: 1px solid var(--rule);
  }

  .btn {
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

  @media (max-width: 560px) {
    .d-head h1 { font-size: 26px; }
    .d-body { font-size: 14.5px; }
    .d-attachment { grid-template-columns: 1fr; }
    .d-attachment img,
    .d-attachment-placeholder { aspect-ratio: 16 / 9; }
  }
</style>
