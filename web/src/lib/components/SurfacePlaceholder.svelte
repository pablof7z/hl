<script lang="ts">
  import type { SurfaceSpec } from '$lib/highlighter/surfaces';

  let { spec }: { spec: SurfaceSpec } = $props();
</script>

<section class="surface-shell">
  <header class="surface-header">
    <div class="surface-header-copy">
      <h1 class="surface-title">{spec.title}</h1>
      <p class="surface-description">{spec.description}</p>
    </div>

    <div class="surface-status-card">
      <span class="surface-status-label">Status</span>
      <strong class="surface-status-value">{spec.status}</strong>
    </div>
  </header>

  {#if spec.actions?.length}
    <div class="surface-actions">
      {#each spec.actions as action (action.href)}
        <a
          href={action.href}
          class={`btn ${action.tone === 'secondary' ? 'btn-outline' : 'btn-primary'}`}
        >
          {action.label}
        </a>
      {/each}
    </div>
  {/if}

  <div class="surface-sections">
    {#each spec.sections as section (section.title)}
      <article class="surface-section">
        <h2>{section.title}</h2>
        <ul>
          {#each section.items as item}
            <li>{item}</li>
          {/each}
        </ul>
      </article>
    {/each}
  </div>

  {#if spec.note}
    <p class="surface-note">{spec.note}</p>
  {/if}
</section>

<style>
  .surface-shell {
    display: grid;
    gap: 1.5rem;
    padding: 1.25rem 0 3rem;
  }

  .surface-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 20rem);
    gap: 1.5rem;
    align-items: start;
  }

  .surface-header-copy {
    display: grid;
    gap: 0.75rem;
  }

  .surface-title {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 5vw, 3.25rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .surface-description {
    max-width: 42rem;
    margin: 0;
    color: var(--muted);
    font-size: 1rem;
  }

  .surface-status-card {
    display: grid;
    gap: 0.35rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface-soft);
  }

  .surface-status-label {
    color: var(--muted);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .surface-status-value {
    color: var(--text-strong);
    font-size: 1rem;
  }

  .surface-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .surface-sections {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: 1rem;
  }

  .surface-section {
    display: grid;
    gap: 0.75rem;
    min-width: 0;
    padding: 0;
  }

  .surface-section h2 {
    margin: 0;
    color: var(--text-strong);
    font-size: 1rem;
  }

  .surface-section ul {
    display: grid;
    gap: 0.6rem;
    padding-left: 1.1rem;
    margin: 0;
    color: var(--muted);
  }

  .surface-note {
    margin: 0;
    padding: 1rem 1.1rem;
    border-left: 3px solid var(--accent);
    border-radius: 0 var(--radius-md) var(--radius-md) 0;
    background: color-mix(in srgb, var(--accent) 6%, white);
    color: var(--text);
  }

  @media (max-width: 860px) {
    .surface-header {
      grid-template-columns: 1fr;
    }
  }
</style>
