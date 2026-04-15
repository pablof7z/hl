<script lang="ts">
  import { goto } from '$app/navigation';
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();

  $effect(() => {
    const path = window.location.pathname;
    const parentPath = path.replace(/\/discussion\/?$/, '');
    goto(`${parentPath}?tab=discussion`, { replaceState: true });
  });
</script>

<p style="color: var(--muted); padding: 2rem 0;">Redirecting to discussion…</p>
