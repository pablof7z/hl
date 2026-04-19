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

  <div class="notes-cta">
    <button class="add-note-btn" type="button" onclick={handleAddNote}>
      Add a note
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
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 15px;
    color: var(--ink-fade);
    text-align: center;
    padding: 40px 0;
    margin: 0;
  }

  .notes-cta {
    padding: 20px 0 4px;
    display: flex;
    justify-content: flex-start;
  }

  .add-note-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .add-note-btn:hover {
    text-decoration: underline;
  }

  .add-note-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
