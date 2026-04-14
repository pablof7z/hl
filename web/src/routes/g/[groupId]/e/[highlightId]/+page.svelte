<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { shareHighlightSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Share Highlight — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={shareHighlightSpec} />
