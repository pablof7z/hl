<script lang="ts">
  type Lens = 'community' | 'circles' | 'network';

  let {
    communityName,
    communityUrl,
    activeLens = 'community',
    onLensChange
  }: {
    communityName: string;
    communityUrl: string;
    activeLens?: Lens;
    onLensChange?: (lens: Lens) => void;
  } = $props();

  const lenses = $derived<{ id: Lens; label: string }[]>([
    { id: 'community', label: communityName },
    { id: 'circles', label: 'All my circles' },
    { id: 'network', label: 'My network' }
  ]);
</script>

<div class="context-bar">
  <a href={communityUrl} class="context-back">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M19 12H5" />
      <path d="M12 19l-7-7 7-7" />
    </svg>
    Back to {communityName}
  </a>

  <div class="context-lenses">
    {#each lenses as lens (lens.id)}
      <button
        class="context-lens"
        class:active={activeLens === lens.id}
        onclick={() => onLensChange?.(lens.id)}
      >
        {lens.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .context-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding: 0.75rem 0;
    border-bottom: 1px solid var(--border-light);
    margin-bottom: 0.5rem;
  }

  .context-back {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--muted);
    font-size: 0.88rem;
    font-weight: 500;
    text-decoration: none;
    transition: color 120ms ease;
  }

  .context-back:hover {
    color: var(--text-strong);
  }

  .context-lenses {
    display: flex;
    gap: 0.35rem;
  }

  .context-lens {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border: 1px solid var(--border);
    border-radius: 9999px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .context-lens:hover {
    color: var(--text);
    border-color: var(--text);
  }

  .context-lens.active {
    background: var(--text-strong);
    border-color: var(--text-strong);
    color: #fff;
  }
</style>
