<script lang="ts">
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { artifactPath } from '$lib/ndk/artifacts';
  import type { HydratedHighlight } from '$lib/ndk/highlights';

  let {
    artifact,
    highlight = undefined,
    communityName,
    highlightCount = 0
  }: {
    artifact: ArtifactRecord;
    highlight?: HydratedHighlight | undefined;
    communityName: string;
    highlightCount?: number;
  } = $props();
</script>

<article class="featured-artifact">
  <div class="featured-media">
    {#if artifact.image}
      <img src={artifact.image} alt="" loading="lazy" />
    {:else}
      <div class="featured-fallback">
        <span>{artifact.domain.charAt(0).toUpperCase() || '#'}</span>
      </div>
    {/if}
  </div>

  <div class="featured-copy">
    <div class="featured-topline">
      <p class="eyebrow">Featured Conversation</p>
      <div class="featured-badges">
        <span>{artifact.source}</span>
        <span>{artifact.domain}</span>
        {#if highlightCount > 0}
          <span>{highlightCount} highlight{highlightCount === 1 ? '' : 's'}</span>
        {/if}
      </div>
    </div>

    <h2>{artifact.title}</h2>

    <p class="featured-community">In {communityName}</p>

    {#if artifact.author}
      <p class="featured-author">{artifact.author}</p>
    {/if}

    {#if artifact.note}
      <p class="featured-note">{artifact.note}</p>
    {/if}

    {#if highlight}
      <div class="featured-highlight">
        <blockquote>
          <p>{highlight.quote}</p>
        </blockquote>
      </div>
    {/if}

    <div class="featured-actions">
      <a href={artifactPath(artifact.groupId, artifact.id)}>Open artifact</a>
      <a href={`/community/${artifact.groupId}/content/${artifact.id}/discussion`}>Open discussion</a>
      <a href={artifact.url} target="_blank" rel="noreferrer">Visit source</a>
    </div>
  </div>
</article>

<style>
  .featured-artifact {
    display: grid;
    grid-template-columns: minmax(240px, 360px) minmax(0, 1fr);
    gap: 1.35rem;
    padding: 1.35rem;
    border: 1px solid var(--border);
    border-radius: 1.6rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.12), transparent 34%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(248, 244, 238, 0.96));
  }

  .featured-media,
  .featured-media img,
  .featured-fallback {
    width: 100%;
    height: 100%;
    min-height: 100%;
    border-radius: 1.2rem;
  }

  .featured-media {
    overflow: hidden;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .featured-media img {
    display: block;
    aspect-ratio: 4 / 5;
    object-fit: cover;
  }

  .featured-fallback {
    display: grid;
    place-items: center;
    aspect-ratio: 4 / 5;
    color: var(--accent);
    font-size: 2rem;
    font-weight: 700;
  }

  .featured-copy {
    display: grid;
    align-content: start;
    gap: 0.85rem;
  }

  .featured-topline,
  .featured-badges,
  .featured-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .featured-topline {
    justify-content: space-between;
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.76rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .featured-badges span,
  .featured-actions a {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.82);
    color: var(--text);
    font-size: 0.78rem;
    font-weight: 600;
  }

  .featured-actions a:first-child {
    background: var(--accent);
    color: white;
  }

  h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.02;
    letter-spacing: -0.03em;
  }

  .featured-community,
  .featured-author,
  .featured-note,
  .featured-highlight blockquote p {
    margin: 0;
    line-height: 1.6;
  }

  .featured-community {
    color: var(--text-strong);
    font-weight: 700;
  }

  .featured-author,
  .featured-note {
    color: var(--muted);
  }

  .featured-highlight {
    display: grid;
    gap: 0.55rem;
    padding: 1rem 1.05rem;
    border-radius: 1.2rem;
    background: rgba(255, 255, 255, 0.76);
  }

  .featured-highlight blockquote {
    margin: 0;
    padding: 0 0 0 1rem;
    border-left: 2px solid var(--accent);
  }

  .featured-highlight blockquote p {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.08rem;
  }

  @media (max-width: 820px) {
    .featured-artifact {
      grid-template-columns: 1fr;
    }

    .featured-media img,
    .featured-fallback {
      aspect-ratio: 16 / 10;
    }
  }
</style>
