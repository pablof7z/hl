<script lang="ts">
  import { browser } from '$app/environment';
  import { tick } from 'svelte';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { User } from '$lib/ndk/ui/user';
  import { MarkdownEventContent } from '$lib/ndk/ui/markdown-event-content';
  import '$lib/ndk/components/mention';
  import '$lib/ndk/components/embedded-note';
  import '$lib/ndk/components/embedded-article';
  import { relativeTime } from '$lib/utils/time';
  import { chatFilter, messageFromEvent, publishChatMessage, type ChatMessage } from '../chat';

  // 2 minutes in seconds — consecutive messages within this window collapse the header
  const GROUP_WINDOW_SECS = 120;

  let { groupId, isMember }: { groupId: string; isMember: boolean } = $props();

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  const chatFeed = ndk.$subscribe(() => {
    if (!browser || !groupId) return undefined;
    return {
      filters: [chatFilter(groupId)],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: false
    };
  });

  // Optimistic messages — removed once the relay echo arrives
  let optimistic = $state<(ChatMessage & { failed?: boolean })[]>([]);

  const relayMessages = $derived.by(() => {
    return [...chatFeed.events]
      .map(messageFromEvent)
      .filter((m) => m.content)
      .sort((a, b) => a.createdAt - b.createdAt);
  });

  const allMessages = $derived.by(() => {
    const relayIds = new Set(relayMessages.map((m) => m.eventId));
    const pendingOptimistic = optimistic.filter((m) => !relayIds.has(m.eventId));
    return [...relayMessages, ...pendingOptimistic].sort((a, b) => a.createdAt - b.createdAt);
  });

  // Whether to show the author header for a message at a given index
  function showHeader(index: number): boolean {
    if (index === 0) return true;
    const prev = allMessages[index - 1];
    const curr = allMessages[index];
    if (prev.pubkey !== curr.pubkey) return true;
    if (curr.createdAt > prev.createdAt + GROUP_WINDOW_SECS) return true;
    return false;
  }

  // --- Composer ---
  let composerValue = $state('');
  let replyingTo = $state<ChatMessage | null>(null);
  let sending = $state(false);
  let sendError = $state('');

  let messagesEl = $state<HTMLDivElement | null>(null);

  function scrollToBottom(behavior: ScrollBehavior = 'smooth') {
    if (!messagesEl) return;
    messagesEl.scrollTo({ top: messagesEl.scrollHeight, behavior });
  }

  $effect(() => {
    // Scroll to bottom whenever message count changes
    allMessages.length;
    tick().then(() => scrollToBottom());
  });

  $effect(() => {
    // Instant scroll on initial load
    if (chatFeed.eosed && messagesEl) {
      tick().then(() => scrollToBottom('instant'));
    }
  });

  async function send() {
    const content = composerValue.trim();
    if (!content || sending) return;

    const replySnapshot = replyingTo;
    sending = true;
    sendError = '';
    composerValue = '';
    replyingTo = null;

    // Optimistic insert — use a temporary client-side id
    const tempId = `optimistic-${Date.now()}-${Math.random()}`;
    const tempMessage: ChatMessage & { failed?: boolean } = {
      eventId: tempId,
      pubkey: currentUser?.pubkey ?? '',
      content,
      createdAt: Math.floor(Date.now() / 1000),
      parentEventId: replySnapshot?.eventId ?? null,
      rawEvent: {} as never,
      failed: false
    };
    optimistic = [...optimistic, tempMessage];

    try {
      await ensureClientNdk();
      const published = await publishChatMessage(ndk, {
        groupId,
        content,
        replyTo: replySnapshot ?? undefined
      });
      // Replace the temp optimistic entry with the real event id so dedup works
      optimistic = optimistic.map((m) =>
        m.eventId === tempId ? { ...published, failed: false } : m
      );
    } catch (err) {
      sendError = err instanceof Error ? err.message : 'Could not send message.';
      // Mark the temp message as failed so UI can show retry
      optimistic = optimistic.map((m) =>
        m.eventId === tempId ? { ...m, failed: true } : m
      );
    } finally {
      sending = false;
    }
  }

  async function retry(message: ChatMessage & { failed?: boolean }) {
    optimistic = optimistic.filter((m) => m.eventId !== message.eventId);
    composerValue = message.content;
    replyingTo = message.parentEventId
      ? (allMessages.find((m) => m.eventId === message.parentEventId) ?? null)
      : null;
    await tick();
    await send();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  function shortPubkey(pk: string) {
    return pk ? `${pk.slice(0, 8)}…` : '';
  }
</script>

<div class="chat-tab">
  <!-- Message list -->
  <div class="messages" bind:this={messagesEl}>
    {#if !chatFeed.eosed && allMessages.length === 0}
      <div class="empty-state">
        <p>Loading messages…</p>
      </div>
    {:else if allMessages.length === 0}
      <div class="empty-state">
        <p>No messages yet. {isMember ? 'Say hello.' : 'Join the room to chat.'}</p>
      </div>
    {:else}
      {#each allMessages as message, i (message.eventId)}
        {@const isOwn = message.pubkey === currentUser?.pubkey}
        {@const isFailed = 'failed' in message && message.failed}
        <div class="chat {isOwn ? 'chat-end' : 'chat-start'}" class:opacity-60={isFailed}>
          {#if showHeader(i)}
            <div class="chat-image">
              <User.Root {ndk} pubkey={message.pubkey}>
                <a href="/profile/{message.pubkey}" class="block">
                  <User.Avatar class="!size-8 rounded-full object-cover" />
                </a>
              </User.Root>
            </div>
            <div class="chat-header">
              <User.Root {ndk} pubkey={message.pubkey}>
                <User.Name fallback={shortPubkey(message.pubkey)} class="font-semibold text-sm" />
              </User.Root>
              <time class="text-xs opacity-50 ml-1">{relativeTime(message.createdAt)}</time>
            </div>
          {/if}

          <div class="chat-bubble {isOwn ? 'chat-bubble-primary' : 'bg-base-200 text-base-content'}">
            {#if message.parentEventId}
              {@const parent = allMessages.find((m) => m.eventId === message.parentEventId)}
              {#if parent}
                <div class="reply-quote">
                  <span class="reply-author">
                    <User.Root {ndk} pubkey={parent.pubkey}>
                      <User.Name fallback={shortPubkey(parent.pubkey)} />
                    </User.Root>
                  </span>
                  <span class="reply-text">{parent.content.slice(0, 80)}{parent.content.length > 80 ? '…' : ''}</span>
                </div>
              {/if}
            {/if}

            <MarkdownEventContent
              {ndk}
              content={message.content}
              class="chat-msg-content [overflow-wrap:anywhere]"
            />

            {#if isFailed}
              <div class="flex items-center gap-1 mt-1">
                <span class="text-error text-xs">Failed to send.</span>
                <button
                  type="button"
                  class="btn btn-xs btn-ghost text-error"
                  onclick={() => retry(message as ChatMessage & { failed?: boolean })}
                >
                  Retry
                </button>
              </div>
            {/if}
          </div>

          {#if isMember && !isFailed}
            <div class="chat-footer">
              <button
                type="button"
                class="btn btn-ghost btn-xs opacity-0 group-hover:opacity-100 reply-btn"
                onclick={() => (replyingTo = message)}
              >
                Reply
              </button>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <!-- Composer area -->
  <div class="composer-wrap">
    {#if !currentUser}
      <div class="join-cta">
        <p>Sign in to chat.</p>
      </div>
    {:else if !isMember}
      <div class="join-cta">
        <p>Join the room to participate in chat.</p>
      </div>
    {:else if isReadOnly}
      <div class="join-cta">
        <p>Read-only session — cannot send messages.</p>
      </div>
    {:else}
      {#if replyingTo}
        <div class="replying-to-pill">
          <span>
            Replying to
            <User.Root {ndk} pubkey={replyingTo.pubkey}>
              <User.Name fallback={shortPubkey(replyingTo.pubkey)} class="font-semibold" />
            </User.Root>
          </span>
          <button
            type="button"
            class="btn btn-ghost btn-xs"
            aria-label="Clear reply"
            onclick={() => (replyingTo = null)}
          >
            ×
          </button>
        </div>
      {/if}

      {#if sendError}
        <p class="send-error">{sendError}</p>
      {/if}

      <div class="composer">
        <textarea
          class="textarea textarea-bordered flex-1 min-h-[2.75rem] max-h-40 resize-none"
          placeholder="Message…"
          rows="1"
          maxlength="4000"
          bind:value={composerValue}
          onkeydown={handleKeydown}
          disabled={sending}
        ></textarea>
        <button
          type="button"
          class="btn btn-primary btn-sm self-end"
          disabled={!composerValue.trim() || sending}
          onclick={send}
        >
          {sending ? '…' : 'Send'}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .chat-tab {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 240px);
    min-height: 400px;
    background: var(--color-base-100, #fff);
    border: 1px solid var(--color-base-300);
    border-radius: 0.5rem;
    overflow: hidden;
  }

  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 0.75rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0;
    scroll-behavior: smooth;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-state p {
    margin: 0;
    font-size: 0.875rem;
    opacity: 0.55;
    font-style: italic;
  }

  /* Make each .chat row a group so the reply button can appear on hover */
  .chat {
    position: relative;
  }

  .chat:hover .reply-btn {
    opacity: 1;
  }

  .reply-quote {
    display: flex;
    flex-direction: column;
    gap: 1px;
    border-left: 2px solid currentColor;
    padding: 2px 6px;
    margin-bottom: 4px;
    opacity: 0.65;
    font-size: 0.8rem;
  }

  .reply-author {
    font-weight: 600;
    font-size: 0.78rem;
  }

  .reply-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(.chat-msg-content p) {
    margin: 0 0 0.3em;
  }
  :global(.chat-msg-content p:last-child) {
    margin-bottom: 0;
  }

  .composer-wrap {
    border-top: 1px solid var(--color-base-300);
    padding: 0.6rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    background: var(--color-base-100, #fff);
  }

  .join-cta {
    text-align: center;
    padding: 0.5rem;
  }

  .join-cta p {
    margin: 0;
    font-size: 0.875rem;
    opacity: 0.6;
  }

  .replying-to-pill {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-primary) 10%, transparent);
    color: var(--color-primary);
    font-size: 0.8rem;
    font-weight: 500;
    width: fit-content;
  }

  .send-error {
    margin: 0;
    color: var(--color-error);
    font-size: 0.82rem;
  }

  .composer {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .reply-btn {
    opacity: 0;
    transition: opacity 0.15s;
  }
</style>
