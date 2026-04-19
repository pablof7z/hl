<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    timestamp,
    memberColorIndex,
    memberName,
    note = ''
  }: {
    timestamp: string;
    memberColorIndex: number;
    memberName: string;
    note?: string;
  } = $props();

  const TINT_VARS = [
    'var(--h-amber)',
    'var(--h-sage)',
    'var(--h-blue)',
    'var(--h-rose)',
    'var(--h-lilac)',
    'var(--h-amber-l)'
  ] as const;

  const borderColor = $derived(TINT_VARS[((memberColorIndex - 1) % 6 + 6) % 6]);
</script>

<div class="timeline-stamp" style:border-left-color={borderColor}>
  <div class="stamp-left">
    <MemberDot colorIndex={memberColorIndex} size="sm" />
    <span class="stamp-name">{memberName}</span>
  </div>
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
