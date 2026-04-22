<script lang="ts">
  import { DropdownMenu as DropdownMenuPrimitive, type DropdownMenuContentProps } from 'bits-ui';
  import { cn } from '$lib/ndk/utils/cn';

  let {
    class: className = '',
    children,
    align = 'end',
    sideOffset = 10,
    collisionPadding = 12,
    ...restProps
  }: DropdownMenuContentProps = $props();
</script>

<DropdownMenuPrimitive.Portal>
  <DropdownMenuPrimitive.Content
    {...restProps}
    {align}
    {sideOffset}
    {collisionPadding}
    class={cn(
      'z-40 min-w-56 overflow-hidden rounded-box border border-base-300 bg-base-100 p-1 shadow-lg',
      className
    )}
  >
    {@render children?.()}
  </DropdownMenuPrimitive.Content>
</DropdownMenuPrimitive.Portal>
