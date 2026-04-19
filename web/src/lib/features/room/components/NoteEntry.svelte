<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    id,
    memberColorIndex,
    memberName,
    content,
    timestamp
  }: {
    id: string;
    memberColorIndex: number;
    memberName: string;
    content: string;
    timestamp: string;
  } = $props();
</script>

<div class="note-entry" data-id={id}>
  <div class="note-header">
    <div class="note-member" aria-hidden="true">
      <MemberDot colorIndex={memberColorIndex} size="sm" />
    </div>
    <span class="note-name">{memberName}</span>
    <span class="note-timestamp">{timestamp}</span>
  </div>
  <p class="note-content">{content}</p>
</div>

<style>
  .note-entry {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 0;
    border-bottom: 1px solid var(--rule-soft);
  }

  .note-entry:last-child {
    border-bottom: none;
  }

  .note-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .note-member {
    flex-shrink: 0;
  }

  .note-name {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
    flex: 1;
    min-width: 0;
  }

  .note-timestamp {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    flex-shrink: 0;
  }

  .note-content {
    font-family: var(--font-serif);
    font-size: 15px;
    font-weight: 400;
    color: var(--ink-soft);
    line-height: 1.6;
    margin: 0;
  }
</style>
