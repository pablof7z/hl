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

<a class="artifact-mini-card" href={artifactPath(artifact.groupId, artifact.id)}>
  <div class="artifact-mini-media">
    {#if artifact.image}
      <img src={artifact.image} alt="" loading="lazy" />
    {:else}
      <div class="artifact-mini-fallback">
        <span>{artifact.domain.charAt(0).toUpperCase() || '#'}</span>
      </div>
    {/if}
  </div>

  <div class="artifact-mini-copy">
    <strong>{artifact.title}</strong>
    <p>{artifact.source} · {artifact.domain}</p>
    {#if highlightCount > 0}
      <span>{highlightCount} highlight{highlightCount === 1 ? '' : 's'}</span>
    {/if}
  </div>
</a>

<style>
  .artifact-mini-card {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr);
    gap: 0.8rem;
    align-items: center;
    color: inherit;
  }

  .artifact-mini-media,
  .artifact-mini-media img,
  .artifact-mini-fallback {
    width: 100%;
    aspect-ratio: 4 / 5;
    border-radius: 0.95rem;
  }

  .artifact-mini-media {
    overflow: hidden;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .artifact-mini-media img {
    object-fit: cover;
  }

  .artifact-mini-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-weight: 700;
  }

  .artifact-mini-copy {
    display: grid;
    gap: 0.2rem;
    min-width: 0;
  }

  .artifact-mini-copy strong,
  .artifact-mini-copy p,
  .artifact-mini-copy span {
    margin: 0;
  }

  .artifact-mini-copy strong {
    color: var(--text-strong);
    font-size: 0.98rem;
    line-height: 1.3;
  }

  .artifact-mini-copy p {
    color: var(--muted);
    font-size: 0.82rem;
    line-height: 1.5;
  }

  .artifact-mini-copy span {
    color: var(--accent);
    font-size: 0.78rem;
    font-weight: 700;
  }
</style>
