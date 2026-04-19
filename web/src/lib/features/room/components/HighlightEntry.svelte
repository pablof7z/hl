<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    id,
    memberColorIndex,
    memberName,
    quote,
    timestamp
  }: {
    id: string;
    memberColorIndex: number;
    memberName: string;
    quote: string;
    timestamp: string;
  } = $props();
</script>

<div class="highlight-entry" data-id={id}>
  <div class="highlight-body">
    <div class="highlight-header">
      <div class="highlight-member" aria-hidden="true">
        <MemberDot colorIndex={memberColorIndex} size="sm" />
      </div>
      <span class="highlight-name">{memberName}</span>
      <span class="highlight-timestamp">{timestamp}</span>
    </div>
    <p class="highlight-quote">{quote}</p>
  </div>
</div>

<style>
  .highlight-entry {
    display: flex;
    padding: 16px 0;
    border-bottom: 1px solid var(--rule-soft);
  }

  .highlight-entry:hover {
    background-color: var(--surface-muted);
  }

  .highlight-entry:last-child {
    border-bottom: none;
  }

  .highlight-body {
    border-left: 3px solid var(--marker-strong);
    padding-left: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .highlight-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .highlight-member {
    flex-shrink: 0;
  }

  .highlight-name {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    flex: 1;
    min-width: 0;
  }

  .highlight-timestamp {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    flex-shrink: 0;
  }

  .highlight-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 15px;
    color: var(--ink-soft);
    line-height: 1.55;
    margin: 0;
  }
</style>
