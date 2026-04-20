<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import MemberDot from './MemberDot.svelte';

  interface Member {
    pubkey: string;
    colorIndex: number;
  }

  let {
    members,
    inviteHref = '#'
  }: {
    members: Member[];
    inviteHref?: string;
  } = $props();
</script>

<div class="sb-card">
  <div class="sb-head">
    <span>Members · {members.length}</span>
    <a href={inviteHref} class="sb-link">invite another →</a>
  </div>

  {#each members as m (m.pubkey)}
    <User.Root {ndk} pubkey={m.pubkey}>
      <div class="mem-row">
        <MemberDot colorIndex={m.colorIndex} pubkey={m.pubkey} size={32} />
        <div>
          <div class="mem-name">
            <User.Name field="displayName" />
            <span class="handle"><User.Handle /></span>
          </div>
          <div class="mem-status"><User.Bio /></div>
        </div>
      </div>
    </User.Root>
  {/each}
</div>

<style>
  .sb-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 20px 22px;
  }

  .sb-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-fade);
    padding-bottom: 12px;
    border-bottom: 1px dotted var(--rule);
    margin-bottom: 14px;
  }

  .sb-link {
    color: var(--brand-accent);
    text-transform: none;
    letter-spacing: 0.02em;
    font-size: 11px;
    text-decoration: none;
    font-family: var(--font-sans);
    font-weight: 500;
  }

  .sb-link:hover {
    text-decoration: underline;
  }

  .mem-row {
    display: grid;
    grid-template-columns: 34px 1fr;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
    align-items: start;
  }

  .mem-row:last-child {
    border-bottom: none;
  }

  .mem-name {
    font-family: var(--font-sans);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink);
    line-height: 1.2;
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .handle {
    font-weight: 400;
    color: var(--ink-fade);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .mem-status :global(p) {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 13px;
    line-height: 1.4;
    color: var(--ink-soft);
    margin: 2px 0 0;
  }
</style>
