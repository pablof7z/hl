<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  let {
    id,
    pubkey,
    memberColorIndex,
    title,
    body,
    date,
    replies,
    replyHref = '#'
  }: {
    id: string;
    pubkey: string;
    memberColorIndex: number;
    title?: string;
    body: string;
    date?: string;
    replies?: number;
    replyHref?: string;
  } = $props();

  const paragraphs = $derived(body.split(/\n{2,}|\n/).filter((p) => p.trim().length > 0));
</script>

<article class="note-entry" data-id={id}>
  <User.Root {ndk} {pubkey}>
    <div class="note-head">
      <span
        class="room-member-avatar"
        style:--mav-size="36px"
        style:--mav-ring={memberTint(memberColorIndex)}
        style:--mav-ring-width="2px"
      >
        <User.Avatar />
      </span>
      <div class="note-head-meta">
        <div class="note-author">
          <User.Name field="displayName" />
          <span class="handle"><User.Handle /></span>
        </div>
        {#if date || replies !== undefined}
          <div class="note-date">
            {date ?? ''}
            {#if date && replies !== undefined} · {/if}
            {#if replies !== undefined}{replies} {replies === 1 ? 'reply' : 'replies'}{/if}
          </div>
        {/if}
      </div>
    </div>
  </User.Root>

  {#if title}<h4 class="note-title">{title}</h4>{/if}

  <div class="note-body">
    {#each paragraphs as p (p)}
      <p>{p}</p>
    {/each}
  </div>

  {#if replies && replies > 0}
    <div class="note-foot">
      <a href={replyHref}>View {replies} {replies === 1 ? 'reply' : 'replies'} →</a>
    </div>
  {/if}
</article>

<style>
  .note-entry {
    padding: 26px 28px;
    background: var(--surface-warm);
    border-radius: var(--radius);
  }

  @media (max-width: 760px) {
    .note-entry {
      padding: 20px;
    }
  }

  .note-head {
    display: grid;
    grid-template-columns: 36px 1fr;
    gap: 14px;
    margin-bottom: 16px;
    align-items: center;
  }

  .note-head-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .note-author {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 14px;
    color: var(--ink);
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .handle {
    font-weight: 400;
    color: var(--ink-fade);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .note-date {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .note-title {
    font-family: var(--font-sans);
    font-weight: 700;
    font-size: 18px;
    color: var(--ink);
    margin: 0 0 14px;
    letter-spacing: -0.015em;
    line-height: 1.2;
  }

  .note-body {
    font-family: var(--font-serif);
    font-size: 17px;
    line-height: 1.65;
    color: var(--ink);
  }

  .note-body p {
    margin: 0 0 0.85em;
  }

  .note-body p:last-child {
    margin-bottom: 0;
  }

  .note-foot {
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px dashed rgba(21, 19, 15, 0.1);
  }

  .note-foot a {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
  }

  .note-foot a:hover {
    text-decoration: underline;
  }
</style>
