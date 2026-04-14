<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import {
    guestActions,
    launchCards,
    memberActions,
    type SurfaceAction
  } from '$lib/highlighter/surfaces';

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));
  const heroTitle = $derived(
    signedIn ? 'Your reading communities start here.' : 'Reading communities, not content sludge.'
  );
  const heroDescription = $derived(
    signedIn
      ? 'Use the app shell to move between communities, your vault, and the first public share surfaces while the NIP-29 flows come online.'
      : 'Highlighter is being rebuilt around shared sources, highlights, and discussions inside Nostr-native groups. This deploy establishes the app routes and public share surfaces.'
  );
  const actions = $derived((signedIn ? memberActions : guestActions) as SurfaceAction[]);
</script>

<section class="home-hero">
  <div class="home-hero-copy">
    <p class="home-eyebrow">Milestone 1</p>
    <h1>{heroTitle}</h1>
    <p class="home-summary">{heroDescription}</p>

    <div class="home-actions">
      {#each actions as action (action.href)}
        <a
          href={action.href}
          class={`btn ${action.tone === 'secondary' ? 'btn-outline' : 'btn-primary'}`}
        >
          {action.label}
        </a>
      {/each}
    </div>
  </div>

  <aside class="home-status">
    <span class="home-status-label">Implementation focus</span>
    <strong>Foundation routes, auth-aware navigation, and public share URLs</strong>
    <p>
      The app is now pointed at Highlighter surfaces instead of the template article feed. Live
      Nostr data comes next.
    </p>
  </aside>
</section>

<section class="launch-grid">
  {#each launchCards as card (card.href)}
    <a class="launch-card" href={card.href}>
      <span class="launch-card-status">{card.status}</span>
      <h2>{card.label}</h2>
      <p>{card.description}</p>
    </a>
  {/each}
</section>

<section class="foundation-notes">
  <article>
    <h2>What changed in this slice</h2>
    <ul>
      <li>The landing page now speaks in Highlighter terms instead of the upstream article template.</li>
      <li>Protected `/me/*` and public share surfaces are part of the visible route map.</li>
      <li>The new public highlight shape is `/g/[group-id]/e/[highlight-id]`.</li>
    </ul>
  </article>

  <article>
    <h2>What comes next</h2>
    <ul>
      <li>Wire community membership and metadata into `/community` and `/community/[id]`.</li>
      <li>Replace route scaffolds with source, highlight, and discussion data flows.</li>
      <li>Ship the first relay-backed deploy once local verification stays clean.</li>
    </ul>
  </article>
</section>

<style>
  .home-hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 22rem);
    gap: 1.5rem;
    padding: 1.5rem 0 1rem;
    align-items: start;
  }

  .home-hero-copy {
    display: grid;
    gap: 0.9rem;
  }

  .home-eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2.4rem, 7vw, 4.6rem);
    line-height: 0.95;
    letter-spacing: -0.05em;
    max-width: 11ch;
  }

  .home-summary {
    max-width: 44rem;
    margin: 0;
    color: var(--muted);
    font-size: 1.02rem;
  }

  .home-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    padding-top: 0.2rem;
  }

  .home-status {
    display: grid;
    gap: 0.45rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background:
      radial-gradient(circle at top right, color-mix(in srgb, var(--accent) 14%, white), transparent 45%),
      var(--surface-soft);
  }

  .home-status-label {
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .home-status strong {
    color: var(--text-strong);
    font-size: 1rem;
  }

  .home-status p {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .launch-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: 1rem;
    padding: 1rem 0;
  }

  .launch-card {
    display: grid;
    gap: 0.55rem;
    min-height: 11rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface);
    color: inherit;
    text-decoration: none;
    transition:
      transform 140ms ease,
      border-color 140ms ease,
      box-shadow 140ms ease;
  }

  .launch-card:hover,
  .launch-card:focus-visible {
    transform: translateY(-2px);
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
    box-shadow: 0 12px 30px rgba(17, 17, 17, 0.06);
    outline: none;
  }

  .launch-card-status {
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .launch-card h2 {
    margin: 0;
    color: var(--text-strong);
    font-size: 1.05rem;
  }

  .launch-card p {
    margin: 0;
    color: var(--muted);
    font-size: 0.92rem;
  }

  .foundation-notes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
    gap: 1rem;
    padding: 0.5rem 0 2.5rem;
  }

  .foundation-notes article {
    display: grid;
    gap: 0.8rem;
    padding: 1rem;
    border-radius: 1rem;
    background: var(--surface-soft);
    border: 1px solid var(--border);
  }

  .foundation-notes h2 {
    margin: 0;
    color: var(--text-strong);
    font-size: 1rem;
  }

  .foundation-notes ul {
    display: grid;
    gap: 0.6rem;
    margin: 0;
    padding-left: 1.1rem;
    color: var(--muted);
  }

  @media (max-width: 860px) {
    .home-hero {
      grid-template-columns: 1fr;
    }

    h1 {
      max-width: none;
    }
  }
</style>
