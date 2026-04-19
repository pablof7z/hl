<script lang="ts">
  let { roomTitle }: { roomTitle: string } = $props();

  function handleAddHighlight() {
    console.log(`Add highlight clicked for "${roomTitle}" — stub, full capture flow in post-M9`);
  }

  function handleAddNote() {
    console.log(`Add note clicked for "${roomTitle}" — stub, full capture flow in post-M9`);
  }

  function handleInviteMember() {
    console.log(`Invite member clicked for "${roomTitle}" — stub, full capture flow in post-M9`);
  }
</script>

<div class="capture-card">
  <h3 class="headline">Capture a moment</h3>
  <p class="subtext">Share a passage from something you're reading with the group.</p>

  <div class="actions">
    <button type="button" class="btn-primary" onclick={handleAddHighlight}>Add highlight</button>
    <button type="button" class="btn-secondary" onclick={handleAddNote}>Add note</button>
    <button type="button" class="btn-link" onclick={handleInviteMember}>Invite a member</button>
  </div>
</div>

<style>
  .capture-card {
    background: var(--surface-warm);
    border: 1px solid var(--rule);
    border-radius: var(--radius, 4px);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .headline {
    font-family: var(--font-serif);
    font-size: 22px;
    font-weight: 400;
    color: var(--ink);
    margin: 0;
    line-height: 1.2;
  }

  .subtext {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 400;
    color: var(--ink-soft);
    margin: 0;
    line-height: 1.5;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
  }

  .btn-primary {
    width: 100%;
    padding: 10px 16px;
    background: var(--brand-accent);
    color: var(--surface);
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 600;
    border: none;
    border-radius: var(--radius, 4px);
    cursor: pointer;
    text-align: center;
    transition: none;
  }

  .btn-secondary {
    width: 100%;
    padding: 10px 16px;
    background: var(--surface);
    color: var(--brand-accent);
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 400;
    border: 1px solid var(--rule);
    border-radius: var(--radius, 4px);
    cursor: pointer;
    text-align: center;
    transition: none;
  }

  .btn-link {
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    color: var(--brand-accent);
    cursor: pointer;
    text-align: center;
    text-decoration: underline;
    text-decoration-color: transparent;
    transition: none;
  }

  .btn-link:hover {
    text-decoration-color: var(--brand-accent);
  }
</style>
