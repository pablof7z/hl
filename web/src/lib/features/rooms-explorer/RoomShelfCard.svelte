<script lang="ts">
  import type { RoomSummary } from '$lib/ndk/groups';

  let {
    room,
    width = 148
  }: {
    room: RoomSummary;
    width?: number;
  } = $props();

  function gradientFromSlug(slug: string): string {
    let h = 0;
    for (let i = 0; i < slug.length; i++) {
      h = (h * 31 + slug.charCodeAt(i)) & 0xffff;
    }
    const hue = h % 360;
    return `linear-gradient(145deg, hsl(${hue} 32% 22%), hsl(${(hue + 40) % 360} 26% 34%))`;
  }

  function memberLabel(memberCount: number | null): string {
    if (memberCount === null) return '';
    if (memberCount === 0) return '';
    if (memberCount === 1) return '1 member';
    return `${memberCount} members`;
  }
</script>

<a class="shelf-card" href="/r/{room.id}" style:width="{width}px">
  <div
    class="shelf-cover"
    style:background={room.picture ? undefined : gradientFromSlug(room.id)}
  >
    {#if room.picture}
      <img src={room.picture} alt="" loading="lazy" />
    {:else}
      <span class="shelf-initial">{room.name.trim().charAt(0).toUpperCase()}</span>
    {/if}
  </div>

  <div class="shelf-info">
    <p class="shelf-name">{room.name}</p>
    {#if memberLabel(room.memberCount)}
      <p class="shelf-meta">{memberLabel(room.memberCount)}</p>
    {/if}
    {#if room.about}
      <p class="shelf-about">{room.about}</p>
    {/if}
  </div>
</a>

<style>
  .shelf-card {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    text-decoration: none;
    color: inherit;
    flex-shrink: 0;
    transition: transform 140ms ease;
  }

  .shelf-card:hover {
    transform: translateY(-2px);
  }

  .shelf-cover {
    width: 100%;
    aspect-ratio: 1 / 1;
    border-radius: 0.85rem;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-base-300, rgba(0,0,0,0.08));
  }

  .shelf-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .shelf-initial {
    font-size: 2rem;
    font-weight: 800;
    color: rgba(255, 255, 255, 0.75);
    line-height: 1;
  }

  .shelf-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .shelf-name {
    margin: 0;
    font-size: 0.88rem;
    font-weight: 700;
    line-height: 1.25;
    color: var(--color-base-content, #111);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .shelf-meta {
    margin: 0;
    font-size: 0.72rem;
    color: color-mix(in srgb, var(--color-base-content, #111) 45%, transparent);
  }

  .shelf-about {
    margin: 0;
    font-size: 0.76rem;
    line-height: 1.4;
    color: color-mix(in srgb, var(--color-base-content, #111) 55%, transparent);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
</style>
