<script lang="ts">
  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  let {
    id,
    type,
    title,
    author,
    cover
  }: {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
  } = $props();

  const PLACEHOLDER_ICONS: Record<ArtifactType, string> = {
    book: '📖',
    podcast: '🎙',
    article: '📰',
    essay: '✍',
    video: '▶'
  };

  const icon = $derived(PLACEHOLDER_ICONS[type]);
  const hasCover = $derived(type === 'book' && !!cover);

  function handleClick() {
    console.log('shelf tile clicked:', id);
  }
</script>

<button
  class="shelf-tile"
  data-type={type}
  data-id={id}
  type="button"
  onclick={handleClick}
  aria-label="{title}{author ? ' by ' + author : ''}"
>
  <div class="tile-cover" aria-hidden="true">
    {#if hasCover}
      <img
        class="cover-img"
        src={cover}
        alt=""
        width="60"
        height="90"
        loading="lazy"
      />
    {:else}
      <div class="cover-placeholder cover-placeholder--{type}">
        <span class="cover-icon">{icon}</span>
      </div>
    {/if}
  </div>

  <div class="tile-info">
    <span class="tile-title">{title}</span>
    {#if author}
      <span class="tile-author">{author}</span>
    {/if}
  </div>
</button>

<style>
  .shelf-tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
    width: 100px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    scroll-snap-align: start;
  }

  .shelf-tile:hover .tile-title {
    color: var(--brand-accent);
  }

  .tile-cover {
    flex-shrink: 0;
  }

  .cover-img {
    width: 60px;
    height: 90px;
    border-radius: var(--radius, 4px);
    object-fit: cover;
    display: block;
    border: 1px solid var(--rule);
  }

  .cover-placeholder {
    width: 60px;
    height: 90px;
    border-radius: var(--radius, 4px);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    border: 1px solid var(--rule);
  }

  .cover-placeholder--book {
    background-color: var(--surface-warm);
  }

  .cover-placeholder--podcast {
    background-color: var(--surface);
    color: var(--brand-accent);
  }

  .cover-placeholder--article,
  .cover-placeholder--essay {
    background-color: var(--surface-muted);
  }

  .cover-placeholder--video {
    background-color: var(--surface);
  }

  .tile-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tile-title {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    line-height: 1.3;
    /* max 2 lines */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    transition: color 0s;
  }

  .tile-author {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 400;
    color: var(--ink-fade);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
