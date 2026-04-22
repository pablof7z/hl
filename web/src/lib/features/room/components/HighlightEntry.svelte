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

<div class="border-b border-base-300/50 py-5 last:border-b-0" data-id={id}>
  <User.Root {ndk} pubkey={authorPubkey}>
    <div class="mb-2.5 flex items-center gap-2.5 font-mono text-[10px] uppercase tracking-wider text-base-content/60">
      <span
        class="room-member-avatar"
        style:--mav-size="22px"
        style:--mav-ring={memberTint(colorIndex)}
        style:--mav-ring-width="1.5px"
      >
        <User.Avatar />
      </span>
      {#if location}<span class="font-medium text-primary">{location}</span>{/if}
      {#if date}<span class="ml-auto">{date}</span>{/if}
    </div>
  </User.Root>

  <p class="m-0 mb-2.5 border-l-2 border-accent pl-3.5 font-serif text-lg italic leading-normal text-base-content">
    {quote}
  </p>

  {#if replies && replies > 0}
    <div class="flex items-center gap-3.5">
      <a class="text-xs font-medium text-primary no-underline hover:underline" href={replyHref}>
        ● {replies} {replies === 1 ? 'reply' : 'replies'} →
      </a>
    </div>
  {/if}
</div>
