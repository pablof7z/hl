<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    id,
    title,
    accent,
    meta,
    filters,
    children
  }: {
    id: string;
    title?: string;
    accent?: string;
    meta?: string;
    filters?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section {id} class="block">
  {#if title}
    <div class="block-head">
      <h2>
        {#if accent}
          {title.replace(accent, '').trim()}
          <em>{accent}</em>
        {:else}
          {title}
        {/if}
      </h2>
      {#if meta}<div class="meta">{meta}</div>{/if}
    </div>
  {/if}

  {#if filters}
    {@render filters()}
  {/if}

  {@render children()}
</section>

<style>
  .block {
    margin-bottom: var(--block-spacing);
    scroll-margin-top: var(--scroll-margin);
  }

  .block-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--rule);
    margin-bottom: 24px;
    flex-wrap: wrap;
    gap: 12px;
  }

  .block-head h2 {
    font-family: var(--font-sans);
    font-weight: 700;
    font-size: 19px;
    line-height: 1.15;
    letter-spacing: -0.018em;
    color: var(--ink);
    margin: 0;
  }

  .block-head h2 em {
    font-style: normal;
    color: var(--brand-accent);
    font-weight: 700;
  }

  .meta {
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }
</style>
