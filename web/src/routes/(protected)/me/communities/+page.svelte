<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { meCommunitiesSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>My Communities — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={meCommunitiesSpec} />
