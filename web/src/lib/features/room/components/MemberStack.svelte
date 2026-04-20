<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  let {
    members,
    max = 6
  }: {
    members: Array<{ pubkey: string; colorIndex: number }>;
    max?: number;
  } = $props();

  const visible = $derived(members.slice(0, max));
  const overflow = $derived(members.length > max ? members.length - max : 0);
</script>

<div class="member-stack">
  {#each visible as member, i (member.pubkey)}
    <div class="member-stack-item" style:z-index={i + 1}>
      <User.Root {ndk} pubkey={member.pubkey}>
        <span
          class="room-member-avatar"
          style:--mav-size="36px"
          style:--mav-ring={memberTint(member.colorIndex)}
          style:--mav-ring-width="2px"
        >
          <User.Avatar />
        </span>
      </User.Root>
    </div>
  {/each}

  {#if overflow > 0}
    <div class="member-stack-badge">+{overflow}</div>
  {/if}
</div>

<style>
  .member-stack {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .member-stack-item {
    position: relative;
    margin-left: -18px;
  }

  .member-stack-item:first-child {
    margin-left: 0;
  }

  .member-stack-item :global(.room-member-avatar) {
    box-shadow: 0 0 0 1px var(--bg);
  }

  .member-stack-badge {
    margin-left: 6px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-fade);
    white-space: nowrap;
  }
</style>
