<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  interface Message {
    id: string;
    pubkey: string;
    colorIndex: number;
    time: string;
    body: string;
    isReply?: boolean;
  }

  let {
    title,
    starterPubkey,
    startedAt,
    messages,
    replyPlaceholder = 'Reply in the thread…'
  }: {
    title?: string;
    starterPubkey?: string;
    startedAt?: string;
    messages: Message[];
    replyPlaceholder?: string;
  } = $props();
</script>

<div class="thread">
  {#if title || starterPubkey}
    <div class="thread-title">
      {#if title}{title}{/if}
      {#if starterPubkey}
        · started by
        <User.Root {ndk} pubkey={starterPubkey}>
          <User.Name field="displayName" />
        </User.Root>
      {/if}
      {#if startedAt} · {startedAt}{/if}
    </div>
  {/if}

  {#each messages as msg (msg.id)}
    <User.Root {ndk} pubkey={msg.pubkey}>
      <div class="msg" class:reply={msg.isReply}>
        <div class="avatar">
          <span
            class="room-member-avatar"
            style:--mav-size="30px"
            style:--mav-ring={memberTint(msg.colorIndex)}
            style:--mav-ring-width="1.5px"
          >
            <User.Avatar />
          </span>
        </div>
        <div class="msg-body">
          <div class="msg-head">
            <span class="msg-name"><User.Name field="displayName" /></span>
            <span class="msg-handle"><User.Handle /></span>
            <span class="msg-time">{msg.time}</span>
          </div>
          <div class="msg-text">{msg.body}</div>
        </div>
      </div>
    </User.Root>
  {/each}

  <div class="thread-reply-box">
    <span>{replyPlaceholder}</span>
    <button type="button" class="btn btn-sm btn-neutral ml-auto">Send</button>
  </div>
</div>

<style>
  .thread {
    background: var(--surface-warm);
    border-radius: var(--radius);
    padding: 20px 24px;
    margin-top: 8px;
  }

  .thread-title {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-fade);
    margin-bottom: 14px;
  }

  .msg {
    display: grid;
    grid-template-columns: 30px 1fr;
    gap: 14px;
    padding: 10px 0;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
  }

  .msg:last-of-type {
    border-bottom: none;
  }

  .msg.reply {
    padding-left: 22px;
    border-left: 1px dotted rgba(21, 19, 15, 0.15);
    margin-left: 15px;
  }

  .avatar {
    display: flex;
    align-items: flex-start;
  }

  .msg-body {
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--ink);
    min-width: 0;
  }

  .msg-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 3px;
    flex-wrap: wrap;
  }

  .msg-name {
    font-weight: 600;
    color: var(--ink);
  }

  .msg-handle {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
  }

  .msg-time {
    font-size: 11px;
    color: var(--ink-fade);
    margin-left: auto;
    font-family: var(--font-mono);
    letter-spacing: 0.04em;
  }

  .msg-text {
    color: var(--ink-soft);
  }

  .thread-reply-box {
    margin-top: 12px;
    padding: 10px 14px;
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--ink-fade);
    font-family: var(--font-sans);
  }

</style>
