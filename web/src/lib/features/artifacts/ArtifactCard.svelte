<script lang="ts">
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { artifactPath } from '$lib/ndk/artifacts';

  let {
    artifact,
    highlightCount = 0
  }: {
    artifact: ArtifactRecord;
    highlightCount?: number;
  } = $props();
</script>

<a class="artifact-card" href={artifactPath(artifact.groupId, artifact.id)}>
  <div class="artifact-card-media">
    {#if artifact.image}
      <img src={artifact.image} alt="" loading="lazy" />
    {:else}
      <div class="artifact-card-fallback">
        <span>{artifact.domain.charAt(0).toUpperCase() || '#'}</span>
      </div>
    {/if}
  </div>

  <div class="artifact-card-copy">
    <div class="artifact-card-topline">
      <span class="artifact-source">{artifact.source}</span>
      <span class="artifact-domain">{artifact.domain}</span>
    </div>

    <h3>{artifact.title}</h3>

    {#if artifact.author}
      <p class="artifact-author">{artifact.author}</p>
    {/if}

    {#if artifact.note}
      <p class="artifact-note">{artifact.note}</p>
    {/if}

    <div class="artifact-card-footer">
      {#if highlightCount > 0}
        <span>{highlightCount} highlight{highlightCount === 1 ? '' : 's'}</span>
      {/if}
      <span>/community/{artifact.groupId}/content/{artifact.id}</span>
    </div>
  </div>
</a>

<style>
  .artifact-card {
    display: grid;
    grid-template-columns: minmax(96px, 132px) minmax(0, 1fr);
    gap: 1rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1.25rem;
    background: var(--surface);
    color: inherit;
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .artifact-card:hover {
    border-color: rgba(255, 103, 25, 0.28);
    transform: translateY(-1px);
    box-shadow: 0 18px 36px rgba(17, 17, 17, 0.06);
  }

  .artifact-card-media {
    width: 100%;
    aspect-ratio: 4 / 5;
    overflow: hidden;
    border-radius: 0.95rem;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .artifact-card-media img,
  .artifact-card-fallback {
    width: 100%;
    height: 100%;
  }

  .artifact-card-media img {
    object-fit: cover;
  }

  .artifact-card-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 1.5rem;
    font-weight: 700;
  }

  .artifact-card-copy {
    display: grid;
    gap: 0.55rem;
    min-width: 0;
  }

  .artifact-card-topline,
  .artifact-card-footer {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .artifact-source,
  .artifact-domain,
  .artifact-card-footer span {
    display: inline-flex;
    align-items: center;
    min-height: 1.8rem;
    padding: 0 0.55rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
  }

  h3 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.2rem;
    line-height: 1.2;
    letter-spacing: -0.02em;
  }

  .artifact-author,
  .artifact-note {
    margin: 0;
    color: var(--muted);
    line-height: 1.55;
  }

  .artifact-note {
    color: var(--text);
  }

  @media (max-width: 720px) {
    .artifact-card {
      grid-template-columns: 1fr;
    }

    .artifact-card-media {
      aspect-ratio: 16 / 9;
    }
  }
</style>
