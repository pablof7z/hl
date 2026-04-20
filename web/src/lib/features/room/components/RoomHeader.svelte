<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import MemberDot from './MemberDot.svelte';

  interface Member {
    pubkey: string;
    colorIndex: number;
  }

  let {
    title,
    members
  }: {
    title: string;
    members: Member[];
  } = $props();
</script>

<section class="room-id">
  <h1 class="room-title">{title}</h1>

  <div class="room-members-row">
    <div class="stack">
      {#each members.slice(0, 6) as member, i (member.pubkey)}
        <span class:overlap={i > 0}>
          <User.Root {ndk} pubkey={member.pubkey}>
            <MemberDot colorIndex={member.colorIndex} pubkey={member.pubkey} size={36} />
          </User.Root>
        </span>
      {/each}
    </div>
  </div>
</section>

<style>
  .room-id {
    padding: 56px 0 36px;
    border-bottom: 1px solid var(--rule);
  }

  .room-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(44px, 6vw, 68px);
    line-height: 1.02;
    letter-spacing: -0.025em;
    color: var(--ink);
    margin: 0 0 32px;
  }

  .room-members-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .stack {
    display: flex;
  }

  .overlap {
    margin-left: -10px;
  }

  .stack > span :global(.member-dot) {
    border: 2.5px solid var(--bg);
  }
</style>
