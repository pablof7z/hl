<script lang="ts">
  import type { RoomSummary } from '$lib/ndk/groups';

  let {
    room,
    joined = false,
    showRoute = true
  }: {
    room: RoomSummary;
    joined?: boolean;
    showRoute?: boolean;
  } = $props();

  function initialFor(name: string): string {
    return name.trim().charAt(0).toUpperCase() || '#';
  }

  function memberLabel(memberCount: number | null): string {
    if (memberCount === null) return 'Private membership';
    if (memberCount === 1) return '1 member';
    return `${memberCount} members`;
  }
</script>

<article class="room-card">
  <a class="room-card-link" href={`/r/${room.id}`} aria-label={room.name}>
    <div class="room-card-media">
      {#if room.picture}
        <img src={room.picture} alt="" loading="lazy" />
      {:else}
        <span>{initialFor(room.name)}</span>
      {/if}
    </div>

    <div class="room-card-body">
      <div class="room-card-topline">
        <p class="room-card-title">{room.name}</p>
        <div class="room-badges">
          {#if joined}
            <span class="joined-badge">Joined</span>
          {/if}
          <span>{room.visibility}</span>
          <span>{room.access}</span>
        </div>
      </div>

      <p class="room-card-about">
        {room.about || 'No description yet. This group is live on the relay and ready for sources, highlights, and discussion.'}
      </p>

      <div class="room-card-meta">
        <span>{memberLabel(room.memberCount)}</span>
        {#if showRoute}
          <span>/r/{room.id}</span>
        {/if}
      </div>
    </div>
  </a>
</article>

<style>
  .room-card {
    border: 1px solid var(--color-base-300);
    border-radius: 1.4rem;
    background: var(--surface);
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .room-card:hover,
  .room-card:focus-within {
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    transform: translateY(-1px);
    box-shadow: 0 16px 40px rgba(17, 17, 17, 0.06);
  }

  .room-card-link {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1rem;
    min-height: 100%;
    padding: 1rem;
    color: inherit;
    text-decoration: none;
  }

  .room-card-media {
    display: grid;
    place-items: center;
    width: 3.25rem;
    height: 3.25rem;
    border-radius: 1rem;
    background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 14%, transparent), color-mix(in srgb, var(--accent) 4%, transparent));
    overflow: hidden;
    color: var(--accent);
    font-size: 1.1rem;
    font-weight: 700;
  }

  .room-card-media img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .room-card-body {
    display: grid;
    gap: 0.7rem;
    min-width: 0;
  }

  .room-card-topline {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: 0.75rem;
  }

  .room-card-title {
    margin: 0;
    color: var(--text-strong);
    font-size: 1rem;
    font-weight: 700;
    line-height: 1.3;
  }

  .room-badges {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    justify-content: end;
  }

  .room-badges span,
  .room-card-meta span {
    display: inline-flex;
    align-items: center;
    min-height: 1.75rem;
    padding: 0 0.55rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
  }

  .room-badges .joined-badge {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

  .room-card-about {
    margin: 0;
    color: var(--muted);
    font-size: 0.92rem;
    line-height: 1.55;
  }

  .room-card-meta {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }
</style>
