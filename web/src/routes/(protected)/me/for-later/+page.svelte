<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { meForLaterSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>For Later — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={meForLaterSpec} />
