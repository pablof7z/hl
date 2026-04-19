<script lang="ts">
  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  let {
    id,
    type,
    title,
    author,
    cover,
    highlightCount = 0,
    discussionCount = 0,
    onArtifactClick
  }: {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
    highlightCount?: number;
    discussionCount?: number;
    onArtifactClick?: (artifact: { id: string; type: ArtifactType; title: string; author?: string; cover?: string; highlightCount?: number; discussionCount?: number }) => void;
  } = $props();

  function handleClick() {
    if (onArtifactClick) {
      onArtifactClick({ id, type, title, author, cover, highlightCount, discussionCount });
    } else {
      console.log('artifact clicked:', id);
    }
  }

  const PLACEHOLDER_ICONS: Record<ArtifactType, string> = {
    book: '📖',
    podcast: '🎙',
    article: '📰',
    essay: '✍',
    video: '▶'
  };

  const icon = $derived(PLACEHOLDER_ICONS[type]);
  const hasCover = $derived(type === 'book' && !!cover);
</script>

<button
  class="artifact-card"
  class:has-cover={hasCover}
  data-type={type}
  data-id={id}
  type="button"
  onclick={handleClick}
  aria-label="{title}{author ? ' by ' + author : ''}"
>
  <div class="artifact-cover" aria-hidden="true">
    {#if hasCover}
      <img
        class="cover-img"
        src={cover}
        alt=""
        width="80"
        height="120"
        loading="lazy"
      />
    {:else}
      <div class="cover-placeholder cover-placeholder--{type}">
        <span class="cover-icon">{icon}</span>
      </div>
    {/if}
  </div>

  <div class="artifact-info">
    <span class="artifact-kicker">THIS WEEK</span>
    <span class="artifact-title">{title}</span>
    {#if author}
      <span class="artifact-author">{author}</span>
    {/if}
    <span class="artifact-counts">{highlightCount} highlights · {discussionCount} discussions</span>
  </div>
</button>

<style>
  .artifact-card {
    display: flex;
    gap: 16px;
    background-color: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius, 4px);
    padding: 16px;
    cursor: pointer;
    text-align: left;
    transition: transform 0s, border-color 0s;
    width: 100%;
  }

  .artifact-card:hover {
    transform: translateY(-2px);
    border-color: var(--brand-accent);
  }

  .artifact-card:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
  }

  .artifact-cover {
    flex-shrink: 0;
  }

  .cover-img {
    width: 80px;
    height: 120px;
    border-radius: var(--radius, 4px);
    object-fit: cover;
    display: block;
  }

  .cover-placeholder {
    width: 80px;
    height: 120px;
    border-radius: var(--radius, 4px);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 28px;
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

  .artifact-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .artifact-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .artifact-title {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 600;
    color: var(--ink);
    line-height: 1.3;
    /* max 2 lines */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-top: 2px;
  }

  .artifact-author {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 400;
    color: var(--ink-soft);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artifact-counts {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    margin-top: auto;
  }
</style>
