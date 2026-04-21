<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';

  type Status = 'active' | 'closed';

  let {
    id,
    status = 'active',
    statusLabel,
    title,
    starterPubkey,
    replies,
    lastAt,
    href = '#'
  }: {
    id?: string;
    status?: Status;
    statusLabel?: string;
    title: string;
    starterPubkey?: string;
    replies: number;
    lastAt: string;
    href?: string;
  } = $props();
</script>

<a {href} class="sdr" data-id={id}>
  <span class="sdr-status {status}">{statusLabel ?? (status === 'active' ? 'Active' : 'Closed')}</span>
  <div class="sdr-title">{title}</div>
  {#if starterPubkey}
    <div class="sdr-source">
      started by
      <User.Root {ndk} pubkey={starterPubkey}>
        <b><User.Name field="displayName" /></b>
      </User.Root>
    </div>
  {/if}
  <div class="sdr-meta">
    <span class="sdr-replies">
      {#if status === 'active'}<span class="hot">●</span>{/if}
      <b>{replies}</b> {replies === 1 ? 'reply' : 'replies'}
    </span>
    <span class="sdr-dot" aria-hidden="true">·</span>
    <span class="sdr-last">{lastAt}</span>
  </div>
</a>

<style>
  .sdr {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 0;
    border-bottom: 1px solid var(--rule-soft);
    text-decoration: none;
    color: inherit;
    transition: background 150ms;
  }

  .sdr:last-child {
    border-bottom: none;
  }

  .sdr-status {
    align-self: flex-start;
    padding: 1px 6px;
    border-radius: 2px;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-weight: 500;
    margin-bottom: 2px;
  }

  .sdr-status.active {
    background: rgba(124, 174, 122, 0.18);
    color: #4A7248;
  }

  .sdr-status.closed {
    background: var(--surface-muted);
    color: var(--ink-fade);
  }

  .sdr-title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 13.5px;
    color: var(--ink);
    line-height: 1.35;
  }

  .sdr-source {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 12px;
    color: var(--ink-fade);
  }

  .sdr-source :global(b) {
    color: var(--ink);
    font-weight: 600;
    font-style: normal;
  }

  .sdr-meta {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-top: 2px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
  }

  .sdr-replies {
    color: var(--ink);
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
  }

  .hot {
    color: var(--brand-accent);
  }

  .sdr-dot {
    color: var(--ink-fade);
  }

  .sdr-last {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
</style>
