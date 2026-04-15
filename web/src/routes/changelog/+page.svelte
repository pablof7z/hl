<script lang="ts">
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();
</script>

<section class="changelog">
  <h1>Changelog</h1>
  <p class="subtitle">Recent commits deployed to beta</p>

  <ul class="commit-list">
    {#each data.commits as commit}
      <li class="commit-item">
        <code class="commit-hash">{commit.shortHash}</code>
        <span class="commit-message">{commit.message}</span>
        <time class="commit-date" datetime={commit.date}>
          {new Date(commit.date).toLocaleDateString('en-US', {
            month: 'short',
            day: 'numeric',
            year: 'numeric'
          })}
        </time>
      </li>
    {/each}
  </ul>
</section>

<style>
  .changelog {
    max-width: 48rem;
  }

  h1 {
    font-family: var(--font-serif);
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-strong);
    margin-bottom: 0.25rem;
  }

  .subtitle {
    color: var(--muted);
    font-size: 0.95rem;
    margin-bottom: 2rem;
  }

  .commit-list {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .commit-item {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    padding: 0.6rem 0;
    border-bottom: 1px solid var(--border-light);
  }

  .commit-hash {
    font-size: 0.82rem;
    color: var(--accent);
    flex-shrink: 0;
  }

  .commit-message {
    flex: 1;
    font-size: 0.92rem;
    color: var(--text-strong);
  }

  .commit-date {
    font-size: 0.8rem;
    color: var(--muted);
    flex-shrink: 0;
  }
</style>
