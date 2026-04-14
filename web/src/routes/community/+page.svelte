<script lang="ts">
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);

  function initialFor(name: string): string {
    return name.trim().charAt(0).toUpperCase() || '#';
  }

  function memberLabel(memberCount: number | null): string {
    if (memberCount === null) return 'Private membership';
    if (memberCount === 1) return '1 member';
    return `${memberCount} members`;
  }
</script>

<svelte:head>
  <title>Communities — Highlighter</title>
</svelte:head>

<section class="community-index">
  <header class="community-index-header">
    <div class="community-index-copy">
      <p class="eyebrow">Communities</p>
      <h1>Reading groups live on the relay now.</h1>
      <p class="lede">
        Browse the NIP-29 communities already indexed on Highlighter and jump into the one you
        want to build around.
      </p>
    </div>

    <a class="create-link" href="/community/create">
      {currentUser ? 'Create community' : 'Sign in to create'}
    </a>
  </header>

  {#if data.communities.length === 0}
    <section class="empty-state">
      <p class="empty-label">No communities have been created on the relay yet.</p>
      <p class="empty-copy">
        The creation flow is live. Publish the first group and it will appear here as soon as the
        relay emits its `kind:39000` metadata.
      </p>
      <a class="empty-cta" href="/community/create">Create the first community</a>
    </section>
  {:else}
    <div class="community-grid">
      {#each data.communities as community (community.id)}
        <article class="community-card">
          <a class="community-card-link" href={`/community/${community.id}`} aria-label={community.name}>
            <div class="community-card-media">
              {#if community.picture}
                <img src={community.picture} alt="" loading="lazy" />
              {:else}
                <span>{initialFor(community.name)}</span>
              {/if}
            </div>

            <div class="community-card-body">
              <div class="community-card-topline">
                <p class="community-card-title">{community.name}</p>
                <div class="community-badges">
                  <span>{community.visibility}</span>
                  <span>{community.access}</span>
                </div>
              </div>

              <p class="community-card-about">
                {community.about || 'No description yet. This group has relay-backed metadata and is ready for artifacts, highlights, and discussion.'}
              </p>

              <div class="community-card-meta">
                <span>{memberLabel(community.memberCount)}</span>
                <span>/community/{community.id}</span>
              </div>
            </div>
          </a>
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  .community-index {
    display: grid;
    gap: 2rem;
    padding: 2.25rem 0 3rem;
  }

  .community-index-header {
    display: flex;
    justify-content: space-between;
    align-items: end;
    gap: 1.5rem;
    flex-wrap: wrap;
  }

  .community-index-copy {
    max-width: 42rem;
  }

  .eyebrow {
    margin: 0 0 0.5rem;
    color: var(--accent);
    font-size: 0.82rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .lede {
    margin: 0.9rem 0 0;
    max-width: 36rem;
    color: var(--muted);
    font-size: 1rem;
  }

  .create-link,
  .empty-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.9rem;
    padding: 0 1rem;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 600;
    transition: background 120ms ease;
  }

  .create-link:hover,
  .empty-cta:hover {
    background: var(--accent-hover);
  }

  .empty-state {
    display: grid;
    gap: 0.75rem;
    max-width: 42rem;
    padding: 1.75rem;
    border: 1px solid var(--border);
    border-radius: 1.4rem;
    background: linear-gradient(180deg, rgba(255, 103, 25, 0.06), rgba(255, 255, 255, 0));
  }

  .empty-label {
    margin: 0;
    color: var(--text-strong);
    font-size: 1.1rem;
    font-weight: 700;
  }

  .empty-copy {
    margin: 0;
    color: var(--muted);
  }

  .community-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1rem;
  }

  .community-card {
    border: 1px solid var(--border);
    border-radius: 1.4rem;
    background: var(--surface);
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .community-card:hover {
    border-color: rgba(255, 103, 25, 0.3);
    transform: translateY(-1px);
    box-shadow: 0 16px 40px rgba(17, 17, 17, 0.06);
  }

  .community-card-link {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1rem;
    min-height: 100%;
    padding: 1rem;
  }

  .community-card-media {
    display: grid;
    place-items: center;
    width: 3.25rem;
    height: 3.25rem;
    border-radius: 1rem;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.14), rgba(255, 103, 25, 0.04));
    overflow: hidden;
    color: var(--accent);
    font-size: 1.1rem;
    font-weight: 700;
  }

  .community-card-media img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .community-card-body {
    display: grid;
    gap: 0.7rem;
    min-width: 0;
  }

  .community-card-topline {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: 0.75rem;
  }

  .community-card-title {
    margin: 0;
    color: var(--text-strong);
    font-size: 1rem;
    font-weight: 700;
    line-height: 1.3;
  }

  .community-badges {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    justify-content: end;
  }

  .community-badges span,
  .community-card-meta span {
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

  .community-card-about {
    margin: 0;
    color: var(--muted);
    font-size: 0.92rem;
    line-height: 1.55;
  }

  .community-card-meta {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  @media (max-width: 720px) {
    .community-index {
      padding-top: 1.5rem;
    }

    .community-card-link {
      grid-template-columns: 1fr;
    }

    .community-card-media {
      width: 3rem;
      height: 3rem;
    }
  }
</style>
