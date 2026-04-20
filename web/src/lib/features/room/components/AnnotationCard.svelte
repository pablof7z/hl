<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  let {
    pubkey,
    colorIndex,
    highlight
  }: {
    pubkey: string;
    colorIndex: number;
    highlight: string;
  } = $props();
</script>

<div class="annotation-card">
  <User.Root {ndk} {pubkey}>
    <div class="annotation-header">
      <span
        class="room-member-avatar"
        style:--mav-size="22px"
        style:--mav-ring={memberTint(colorIndex)}
        style:--mav-ring-width="1.5px"
      >
        <User.Avatar />
      </span>
      <span class="annotation-name"><User.Name field="displayName" /></span>
    </div>
  </User.Root>
  <p class="annotation-text">{highlight}</p>
</div>

<style>
  .annotation-card {
    border-left: 4px solid var(--brand-accent);
    background-color: var(--surface-warm);
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-radius: 0 var(--radius) var(--radius) 0;
    margin: 4px 0;
  }

  .annotation-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .annotation-name {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink);
  }

  .annotation-text {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 14px;
    color: var(--ink);
    line-height: 1.6;
    margin: 0;
  }
</style>
