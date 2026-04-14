<script lang="ts">
  import SurfacePlaceholder from '$lib/components/SurfacePlaceholder.svelte';
  import { discussionSpec } from '$lib/highlighter/surfaces';
</script>

<svelte:head>
  <title>Discussion — Highlighter</title>
</svelte:head>

<SurfacePlaceholder spec={discussionSpec} />
