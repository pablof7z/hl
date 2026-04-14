<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { discoverSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Discover — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={discoverSpec} />
