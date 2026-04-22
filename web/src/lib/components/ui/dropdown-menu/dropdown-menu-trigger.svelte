<script lang="ts">
  import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
  import type { ComponentProps } from 'svelte';
  import { cn } from '$lib/ndk/utils/cn';

  let { class: className = '', children, ...restProps }: ComponentProps<
    typeof DropdownMenuPrimitive.Trigger
  > = $props();
</script>

<DropdownMenuPrimitive.Trigger
  {...restProps}
  class={cn('cursor-pointer appearance-none border-0 bg-transparent', className)}
>
  {@render children?.()}
</DropdownMenuPrimitive.Trigger>
