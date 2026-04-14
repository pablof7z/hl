<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { meSynthesisSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Synthesis — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={meSynthesisSpec} />
