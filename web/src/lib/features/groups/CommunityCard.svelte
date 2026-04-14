<script lang="ts">
  import type { CommunitySummary } from '$lib/ndk/groups';

  let {
    community,
    joined = false,
    showRoute = true
  }: {
    community: CommunitySummary;
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
          {#if joined}
            <span class="joined-badge">Joined</span>
          {/if}
          <span>{community.visibility}</span>
          <span>{community.access}</span>
        </div>
      </div>

      <p class="community-card-about">
        {community.about || 'No description yet. This group is live on the relay and ready for artifacts, highlights, and discussion.'}
      </p>

      <div class="community-card-meta">
        <span>{memberLabel(community.memberCount)}</span>
        {#if showRoute}
          <span>/community/{community.id}</span>
        {/if}
      </div>
    </div>
  </a>
</article>

<style>
  .community-card {
    border: 1px solid var(--border);
    border-radius: 1.4rem;
    background: var(--surface);
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .community-card:hover,
  .community-card:focus-within {
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
    color: inherit;
    text-decoration: none;
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

  .community-badges .joined-badge {
    background: rgba(255, 103, 25, 0.12);
    color: var(--accent);
  }

  .community-card-about {
    margin: 0;
    color: var(--muted);
    font-size: 0.92rem;
    line-height: 1.55;
  }

  .community-card-meta {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }
</style>
