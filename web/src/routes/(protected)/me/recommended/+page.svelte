<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { meRecommendedSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Recommended — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={meRecommendedSpec} />
