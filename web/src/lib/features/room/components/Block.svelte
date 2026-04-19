<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    id,
    children
  }: {
    id: string;
    children: Snippet;
  } = $props();
</script>

<section {id} class="room-block">
  {@render children()}
</section>

<style>
  .room-block {
    scroll-margin-top: var(--scroll-margin);
    margin-bottom: var(--block-spacing);
  }
</style>
