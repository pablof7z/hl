<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  interface Contribution {
    highlights?: number;
    messages?: number;
    notes?: number;
  }

  interface MemberRow {
    pubkey: string;
    colorIndex: number;
    progressPct?: number;
    progressLabel?: string;
    progressState?: 'inProgress' | 'done' | 'none';
    contribution?: Contribution;
    lastHere?: string;
  }

  let {
    members
  }: {
    members: MemberRow[];
  } = $props();
</script>

<div class="members-table">
  <div class="mt-head">
    <div>Member</div>
    <div>Progress</div>
    <div>Contribution</div>
    <div>Last here</div>
  </div>

  {#each members as m (m.pubkey)}
    <User.Root {ndk} pubkey={m.pubkey}>
      <div class="mt-row">
        <div class="mt-member">
          <span
            class="room-member-avatar"
            style:--mav-size="30px"
            style:--mav-ring={memberTint(m.colorIndex)}
          >
            <User.Avatar />
          </span>
          <div>
            <div class="m-n"><User.Name field="displayName" /></div>
            <div class="m-h"><User.Handle /></div>
          </div>
        </div>

        <div class="mt-progress">
          <div class="mt-progress-bar">
            <div
              class="mt-progress-fill"
              class:done={m.progressState === 'done'}
              class:none={!m.progressState || m.progressState === 'none'}
              style:width="{m.progressPct ?? 0}%"
            ></div>
          </div>
          <div class="mt-progress-label">
            {#if m.progressLabel}{@html m.progressLabel}{:else}<em>—</em>{/if}
          </div>
        </div>

        <div class="mt-contribution">
          {#if m.contribution?.highlights !== undefined}
            <span><b>{m.contribution.highlights}</b> highlights</span>
          {:else}
            <span class="none">—</span>
          {/if}
          {#if m.contribution?.messages !== undefined}
            <span><b>{m.contribution.messages}</b> messages</span>
          {:else}
            <span class="none">—</span>
          {/if}
          {#if m.contribution?.notes !== undefined}
            <span><b>{m.contribution.notes}</b> note{m.contribution.notes === 1 ? '' : 's'}</span>
          {:else}
            <span class="none">—</span>
          {/if}
        </div>

        <div class="mt-last">{m.lastHere ?? ''}</div>
      </div>
    </User.Root>
  {/each}
</div>

<style>
  .members-table {
    padding: 14px 32px 32px;
  }

  @media (max-width: 760px) {
    .members-table {
      padding: 14px 20px 24px;
    }
  }

  .mt-head {
    display: grid;
    grid-template-columns: 1.5fr 2fr 2fr 90px;
    gap: 18px;
    padding: 10px 0;
    border-bottom: 1px solid var(--rule);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  .mt-row {
    display: grid;
    grid-template-columns: 1.5fr 2fr 2fr 90px;
    gap: 18px;
    padding: 16px 0;
    align-items: center;
    border-bottom: 1px dotted rgba(21, 19, 15, 0.08);
    font-size: 13px;
  }

  .mt-row:last-child {
    border-bottom: none;
  }

  .mt-member {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .m-n {
    font-family: var(--font-sans);
    font-weight: 600;
    color: var(--ink);
    font-size: 13px;
  }

  .m-h {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-fade);
    letter-spacing: 0.02em;
  }

  .mt-progress {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .mt-progress-bar {
    height: 4px;
    background: var(--surface-muted);
    border-radius: 2px;
    overflow: hidden;
    max-width: 160px;
  }

  .mt-progress-fill {
    height: 100%;
    background: var(--brand-accent);
  }

  .mt-progress-fill.done {
    background: #7CAE7A;
  }

  .mt-progress-fill.none {
    background: transparent;
  }

  .mt-progress-label {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-soft);
  }

  .mt-progress-label :global(b) {
    color: var(--ink);
    font-weight: 600;
  }

  .mt-progress-label :global(em) {
    font-style: italic;
    color: var(--ink-fade);
    font-family: var(--font-serif);
  }

  .mt-contribution {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    font-family: var(--font-sans);
    font-size: 12.5px;
    color: var(--ink-soft);
  }

  .mt-contribution b {
    color: var(--ink);
    font-weight: 600;
  }

  .mt-contribution .none {
    color: var(--ink-fade);
  }

  .mt-last {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-align: right;
  }

  @media (max-width: 760px) {
    .mt-head { display: none; }
    .mt-row { grid-template-columns: 1fr; gap: 10px; padding: 16px 0; }
    .mt-last { text-align: left; }
  }
</style>
