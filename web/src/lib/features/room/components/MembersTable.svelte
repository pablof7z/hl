<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    members
  }: {
    members: Array<{ colorIndex: number; name: string; joinedAt?: string }>;
  } = $props();
</script>

<table class="members-table">
  <thead>
    <tr>
      <th class="col-avatar" scope="col" aria-label="Avatar"></th>
      <th class="col-name" scope="col">Name</th>
      <th class="col-joined" scope="col">Joined</th>
    </tr>
  </thead>
  <tbody>
    {#each members as member (member.name)}
      <tr class="member-row">
        <td class="cell-avatar" aria-hidden="true">
          <MemberDot colorIndex={member.colorIndex} size="lg" />
        </td>
        <td class="cell-name">{member.name}</td>
        <td class="cell-joined">{member.joinedAt ?? '—'}</td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .members-table {
    width: 100%;
    border-collapse: collapse;
  }

  thead th {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    text-align: left;
    padding: 0 0 10px 0;
    border-bottom: 1px solid var(--rule-soft);
  }

  .col-avatar {
    width: 56px;
  }

  .col-joined {
    width: 120px;
  }

  .member-row:hover td {
    background-color: var(--surface-muted);
  }

  .member-row td {
    padding: 12px 0;
    border-bottom: 1px solid var(--rule-soft);
    vertical-align: middle;
  }

  .member-row:last-child td {
    border-bottom: none;
  }

  .cell-avatar {
    padding-right: 12px;
  }

  .cell-name {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
  }

  .cell-joined {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--ink-fade);
    text-align: right;
  }
</style>
