<script lang="ts">
  import type { RoomSummary } from '$lib/ndk/groups';

  let {
    rooms,
    onSelect
  }: {
    rooms: RoomSummary[];
    onSelect?: (room: RoomSummary) => void;
  } = $props();

  let index = $state(0);
  const room = $derived(rooms[index] ?? rooms[0]);

  function gradientFromSlug(slug: string): string {
    // Deterministic hue from slug characters
    let h = 0;
    for (let i = 0; i < slug.length; i++) {
      h = (h * 31 + slug.charCodeAt(i)) & 0xffff;
    }
    const hue = h % 360;
    return `linear-gradient(145deg, hsl(${hue} 35% 18%), hsl(${(hue + 40) % 360} 28% 30%))`;
  }
</script>

{#if room}
  <div class="hero-shell">
    <div
      class="hero-card"
      style:background={room.picture ? undefined : gradientFromSlug(room.id)}
    >
      {#if room.picture}
        <img class="hero-bg" src={room.picture} alt="" loading="eager" />
      {/if}
      <div class="hero-scrim"></div>

      <div class="hero-body">
        <span class="hero-kicker">Featured</span>
        <h2 class="hero-name">{room.name}</h2>
        {#if room.about}
          <p class="hero-about">{room.about}</p>
        {/if}
        {#if room.memberCount !== null && room.memberCount > 0}
          <p class="hero-meta">{room.memberCount} members</p>
        {/if}
        <div class="hero-actions">
          <a class="btn btn-sm btn-neutral" href="/r/{room.id}">Open room</a>
          {#if onSelect}
            <button class="btn btn-sm btn-ghost text-white/80" onclick={() => onSelect?.(room)}>
              Preview
            </button>
          {/if}
        </div>
      </div>

      {#if rooms.length > 1}
        <div class="hero-dots">
          {#each rooms as _, i (i)}
            <button
              class="hero-dot"
              class:active={i === index}
              aria-label="Show room {i + 1}"
              onclick={() => { index = i; }}
            ></button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .hero-shell {
    width: 100%;
  }

  .hero-card {
    position: relative;
    width: 100%;
    height: 320px;
    border-radius: 1.25rem;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }

  @media (max-width: 640px) {
    .hero-card {
      height: 260px;
      border-radius: 0.75rem;
    }
  }

  .hero-bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center top;
  }

  .hero-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      transparent 0%,
      rgba(0, 0, 0, 0.12) 40%,
      rgba(0, 0, 0, 0.72) 100%
    );
  }

  .hero-body {
    position: relative;
    z-index: 1;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .hero-kicker {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.72);
  }

  .hero-name {
    margin: 0;
    font-size: clamp(1.4rem, 4vw, 1.9rem);
    font-weight: 700;
    line-height: 1.1;
    color: white;
    letter-spacing: -0.02em;
  }

  .hero-about {
    margin: 0.1rem 0 0;
    font-size: 0.88rem;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.80);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hero-meta {
    margin: 0;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.60);
  }

  .hero-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.55rem;
  }

  .hero-dots {
    position: absolute;
    bottom: 1rem;
    right: 1.25rem;
    display: flex;
    gap: 0.35rem;
    z-index: 2;
  }

  .hero-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.35);
    border: none;
    cursor: pointer;
    padding: 0;
    transition: background 180ms ease, width 180ms ease;
  }

  .hero-dot.active {
    background: white;
    width: 18px;
  }
</style>
