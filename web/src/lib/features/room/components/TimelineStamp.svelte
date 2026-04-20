<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  let {
    timestamp,
    pubkey,
    memberColorIndex,
    note = ''
  }: {
    timestamp: string;
    pubkey: string;
    memberColorIndex: number;
    note?: string;
  } = $props();

  const borderColor = $derived(memberTint(memberColorIndex));
</script>

<div class="timeline-stamp" style:border-left-color={borderColor}>
  <User.Root {ndk} {pubkey}>
    <div class="stamp-left">
      <span
        class="room-member-avatar"
        style:--mav-size="24px"
        style:--mav-ring={borderColor}
        style:--mav-ring-width="1.5px"
      >
        <User.Avatar />
      </span>
      <span class="stamp-name"><User.Name field="displayName" /></span>
    </div>
  </User.Root>
  <div class="stamp-right">
    <span class="stamp-time">{timestamp}</span>
    {#if note}
      <p class="stamp-note">{note}</p>
    {/if}
  </div>
</div>

<style>
  .timeline-stamp {
    border-left: 3px solid var(--h-amber);
    padding: 12px 16px;
    background-color: var(--surface-warm);
    display: flex;
    gap: 12px;
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .stamp-left {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    min-width: 60px;
    padding-top: 2px;
  }

  .stamp-name {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-soft);
    text-align: center;
    word-break: break-word;
  }

  .stamp-right {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }

  .stamp-time {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 700;
    color: var(--brand-accent);
    line-height: 1;
  }

  .stamp-note {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 14px;
    color: var(--ink);
    line-height: 1.6;
    margin: 0;
  }
</style>
