<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  let {
    title,
    members
  }: {
    title: string;
    members: Array<{ colorIndex: number; name?: string; initials?: string }>;
  } = $props();

  function deriveInitials(name?: string, fallbackIndex = 1): string {
    if (!name) return `M${fallbackIndex}`;
    const parts = name.trim().split(/\s+/);
    if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
</script>

<section class="room-id">
  <h1 class="room-title">{title}</h1>

  <div class="room-members-row">
    <div class="stack">
      {#each members.slice(0, 6) as member, i (i)}
        <span class:overlap={i > 0}>
          <MemberDot
            colorIndex={member.colorIndex}
            initials={member.initials ?? deriveInitials(member.name, i + 1)}
            size={36}
            title={member.name}
          />
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
