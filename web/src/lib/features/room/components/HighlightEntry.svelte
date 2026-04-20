<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  let {
    id,
    authorPubkey,
    colorIndex,
    quote,
    location,
    date,
    replies,
    replyHref = '#'
  }: {
    id: string;
    authorPubkey: string;
    colorIndex: number;
    quote: string;
    location?: string;
    date?: string;
    replies?: number;
    replyHref?: string;
  } = $props();
</script>

<div class="hl-entry" data-id={id}>
  <User.Root {ndk} pubkey={authorPubkey}>
    <div class="hl-entry-meta">
      <span
        class="room-member-avatar"
        style:--mav-size="22px"
        style:--mav-ring={memberTint(colorIndex)}
        style:--mav-ring-width="1.5px"
      >
        <User.Avatar />
      </span>
      {#if location}<span class="hl-loc">{location}</span>{/if}
      {#if date}<span class="hl-date">{date}</span>{/if}
    </div>
  </User.Root>

  <p class="hl-entry-quote">{quote}</p>

  {#if replies && replies > 0}
    <div class="hl-entry-foot">
      <a class="hl-thread" href={replyHref}>● {replies} {replies === 1 ? 'reply' : 'replies'} →</a>
    </div>
  {/if}
</div>

<style>
  .hl-entry {
    padding: 20px 0;
    border-bottom: 1px solid var(--rule-soft);
  }

  .hl-entry:last-child { border-bottom: none; }

  .hl-entry-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .hl-loc { color: var(--brand-accent); font-weight: 500; }
  .hl-date { margin-left: auto; }

  .hl-entry-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 18px;
    line-height: 1.5;
    color: var(--ink);
    margin: 0 0 10px;
    padding-left: 14px;
    border-left: 2px solid var(--marker-strong);
  }

  .hl-entry-foot {
    display: flex;
    gap: 14px;
    align-items: center;
  }

  .hl-thread {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
  }

  .hl-thread:hover { text-decoration: underline; }
</style>
