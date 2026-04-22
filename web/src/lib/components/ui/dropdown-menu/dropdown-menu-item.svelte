<script lang="ts">
  import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
  import type { ComponentProps } from 'svelte';
  import { cn } from '$lib/ndk/utils/cn';

  interface Props extends ComponentProps<typeof DropdownMenuPrimitive.Item> {
    inset?: boolean;
  }

  let { class: className = '', children, inset = false, ...restProps }: Props = $props();
</script>

<DropdownMenuPrimitive.Item
  {...restProps}
  class={cn(
    'flex cursor-pointer select-none items-center gap-2 rounded-field px-3 py-2 text-sm outline-none transition-colors',
    'data-[highlighted]:bg-base-200 data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50',
    inset && 'pl-10',
    className
  )}
>
  {@render children?.()}
</DropdownMenuPrimitive.Item>
