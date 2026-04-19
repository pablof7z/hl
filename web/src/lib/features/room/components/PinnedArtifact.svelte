<script lang="ts">
  let {
    artifact
  }: {
    artifact: { title: string; author: string; cover: string };
  } = $props();
</script>

<div class="pinned-artifact">
  <div class="artifact-cover-wrap">
    <img
      class="artifact-cover"
      src={artifact.cover}
      alt="Cover of {artifact.title}"
    />
  </div>

  <div class="artifact-body">
    <span class="artifact-kicker">Pinned</span>
    <h2 class="artifact-title">{artifact.title}</h2>
    <p class="artifact-author">{artifact.author}</p>

    <blockquote class="artifact-passage">
      <p>
        "The sovereign individual will operate in ways that would have seemed
        fantastical, or even impossible, to those of the industrial era."
      </p>
    </blockquote>

    <div class="artifact-meta">
      <span class="artifact-discussion-label">Discussions</span>
      <span class="artifact-discussion-count">12</span>
    </div>
  </div>
</div>

<style>
  .pinned-artifact {
    display: flex;
    gap: 24px;
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 24px;
    transition: transform var(--transition), border-color var(--transition);
  }

  .pinned-artifact:hover {
    transform: translateY(-2px);
    border-color: var(--brand-accent);
  }

  /* ── Cover ──────────────────────────────────────────────────── */
  .artifact-cover-wrap {
    flex-shrink: 0;
  }

  .artifact-cover {
    width: 160px;
    height: 240px;
    border-radius: var(--radius);
    object-fit: cover;
    display: block;
  }

  /* ── Body ───────────────────────────────────────────────────── */
  .artifact-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .artifact-kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 400;
    text-transform: uppercase;
    letter-spacing: 0.18em;
    color: var(--ink-fade);
  }

  .artifact-title {
    font-family: var(--font-serif);
    font-size: clamp(28px, 4vw, 42px);
    font-weight: 700;
    line-height: 1.05;
    color: var(--ink);
    margin: 0;
  }

  .artifact-author {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 400;
    color: var(--ink-soft);
    margin: 0;
  }

  /* ── Featured passage ───────────────────────────────────────── */
  .artifact-passage {
    background: var(--marker);
    border-radius: var(--radius);
    padding: 14px 18px;
    margin: 4px 0 0;
  }

  .artifact-passage p {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 16px;
    line-height: 1.6;
    color: var(--ink-soft);
    margin: 0;
  }

  /* ── Responsive ─────────────────────────────────────────────── */
  @media (max-width: 767px) {
    .pinned-artifact {
      flex-direction: column;
      gap: 16px;
    }

    .artifact-cover-wrap {
      width: 100%;
    }

    .artifact-cover {
      width: 100%;
      height: auto;
      max-height: 200px;
      object-fit: cover;
    }
  }

  /* ── Meta ───────────────────────────────────────────────────── */
  .artifact-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: auto;
  }

  .artifact-discussion-label {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 500;
    color: var(--ink-fade);
  }

  .artifact-discussion-count {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 500;
    color: var(--ink);
  }
</style>
