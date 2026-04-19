<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    id,
    memberColorIndex,
    memberName,
    preview,
    replyCount,
    lastActivity
  }: {
    id: string;
    memberColorIndex: number;
    memberName: string;
    preview: string;
    replyCount: number;
    lastActivity: string;
  } = $props();
</script>

<a class="discussion-row" href="/room/{id}">
  <div class="discussion-meta">
    <div class="discussion-author" aria-hidden="true">
      <MemberDot colorIndex={memberColorIndex} size="sm" />
    </div>
    <span class="discussion-name">{memberName}</span>
    <span class="discussion-reply-badge">{replyCount} replies</span>
    <span class="discussion-activity">{lastActivity}</span>
  </div>
  <p class="discussion-preview">{preview}</p>
</a>

<style>
  .discussion-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 0;
    border-bottom: 1px solid var(--rule-soft);
    text-decoration: none;
    color: inherit;
    transition: background-color 0s;
  }

  .discussion-row:hover {
    background-color: var(--surface-muted);
  }

  .discussion-row:last-child {
    border-bottom: none;
  }

  .discussion-meta {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .discussion-author {
    flex-shrink: 0;
  }

  .discussion-name {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
  }

  .discussion-reply-badge {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-fade);
    background-color: var(--surface-muted);
    padding: 2px 7px;
    border-radius: var(--radius-pill, 999px);
    margin-left: 2px;
  }

  .discussion-activity {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    margin-left: auto;
    flex-shrink: 0;
  }

  .discussion-preview {
    font-family: var(--font-serif);
    font-size: 14px;
    font-weight: 400;
    color: var(--ink-soft);
    line-height: 1.5;
    margin: 0;
    /* max 2 lines */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
