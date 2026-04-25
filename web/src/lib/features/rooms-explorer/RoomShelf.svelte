<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    subtitle,
    children
  }: {
    title: string;
    subtitle?: string;
    children: Snippet;
  } = $props();
</script>

<section class="shelf">
  <header class="shelf-header">
    <span class="shelf-title">{title}</span>
    {#if subtitle}
      <span class="shelf-subtitle">{subtitle}</span>
    {/if}
  </header>
  <div class="shelf-scroll">
    <div class="shelf-track">
      {@render children()}
    </div>
  </div>
</section>

<style>
  .shelf {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .shelf-header {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .shelf-title {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-base-content, #111) 50%, transparent);
  }

  .shelf-subtitle {
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--color-base-content, #111);
  }

  .shelf-scroll {
    overflow-x: auto;
    /* hide scrollbar visually but keep scroll functionality */
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  .shelf-scroll::-webkit-scrollbar {
    display: none;
  }

  .shelf-track {
    display: flex;
    gap: 0.85rem;
    padding-bottom: 0.25rem;
  }
</style>
