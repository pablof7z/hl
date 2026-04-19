<script lang="ts">
  import NoteEntry from './NoteEntry.svelte';

  interface NoteEntryProps {
    id: string;
    memberColorIndex: number;
    memberName: string;
    content: string;
    timestamp: string;
  }

  let {
    notes
  }: {
    notes: NoteEntryProps[];
  } = $props();

  function handleAddNote() {
    console.log('stub — full implementation in M5+');
  }

  function handleSeeAll() {
    console.log('see all notes — stub for M5+');
  }
</script>

<div class="notes-tab">
  <div class="notes-list">
    {#if notes.length === 0}
      <p class="empty-state">No notes yet. Start a thread.</p>
    {:else}
      {#each notes as note (note.id)}
        <NoteEntry
          id={note.id}
          memberColorIndex={note.memberColorIndex}
          memberName={note.memberName}
          content={note.content}
          timestamp={note.timestamp}
        />
      {/each}
    {/if}
  </div>

  <div class="notes-footer">
    <button class="add-note-btn" type="button" onclick={handleAddNote}>
      ✎ Write a note
    </button>
    <button class="see-all-link" type="button" onclick={handleSeeAll}>
      See all notes →
    </button>
  </div>
</div>

<style>
  .notes-tab {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .notes-list {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .empty-state {
    font-family: var(--font-sans);
    font-size: 15px;
    color: var(--ink-fade);
    text-align: center;
    padding: 40px 0;
    margin: 0;
  }

  .notes-footer {
    padding: 20px 0 4px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .add-note-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: 1px solid var(--brand-accent);
    padding: 6px 12px;
    border-radius: var(--radius, 4px);
    cursor: pointer;
  }

  .add-note-btn:hover {
    background-color: var(--surface-warm);
  }

  .add-note-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
  }

  .see-all-link {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .see-all-link:hover {
    text-decoration: underline;
  }

  .see-all-link:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
