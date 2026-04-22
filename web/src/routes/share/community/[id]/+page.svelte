<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { shareCommunitySpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Share Room — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={shareCommunitySpec} />
