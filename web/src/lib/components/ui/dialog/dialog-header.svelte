<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/ndk/utils/cn';

  interface Props {
    children?: Snippet;
    class?: string;
  }

  let { children, class: className = '' }: Props = $props();
</script>

<div class={cn('grid gap-2', className)}>
  {@render children?.()}
</div>
