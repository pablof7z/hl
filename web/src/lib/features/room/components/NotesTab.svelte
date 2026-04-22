<script lang="ts">
  import NoteEntry from './NoteEntry.svelte';

  interface NoteRow {
    id: string;
    pubkey: string;
    memberColorIndex: number;
    title?: string;
    content: string;
    date?: string;
    replies?: number;
  }

  let {
    notes,
    onWriteNote
  }: {
    notes: NoteRow[];
    onWriteNote?: () => void;
  } = $props();
</script>

<div class="panel-head">
  <div class="panel-head-note">
    Longer-form reflections from the room. {notes.length} so far.
  </div>
  <button
    type="button"
    class="btn btn-sm"
    onclick={() => onWriteNote?.()}
  >
    ✎ Write a note
  </button>
</div>

<div class="note-list">
  {#if notes.length === 0}
    <p class="empty-state">No notes yet. Start a thread.</p>
  {:else}
    {#each notes as n (n.id)}
      <NoteEntry
        id={n.id}
        pubkey={n.pubkey}
        memberColorIndex={n.memberColorIndex}
        title={n.title}
        body={n.content}
        date={n.date}
        replies={n.replies}
      />
    {/each}
  {/if}
</div>

<style>
  .panel-head {
    padding: 18px 32px 14px;
    border-bottom: 1px solid var(--rule);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  @media (max-width: 760px) {
    .panel-head {
      padding: 14px 20px;
    }
  }

  .panel-head-note {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
    font-weight: 500;
  }

  .note-list {
    padding: 24px 32px 32px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  @media (max-width: 760px) {
    .note-list {
      padding: 20px;
    }
  }

  .empty-state {
    font-family: var(--font-sans);
    font-size: 15px;
    color: var(--ink-fade);
    text-align: center;
    padding: 40px 0;
    margin: 0;
  }
</style>
