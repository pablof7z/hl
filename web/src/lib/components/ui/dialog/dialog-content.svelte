<script lang="ts">
  import { Dialog as DialogPrimitive, type DialogContentProps } from 'bits-ui';
  import { cn } from '$lib/ndk/utils/cn';

  let { class: className = '', children, ...restProps }: DialogContentProps = $props();
</script>

<DialogPrimitive.Portal>
  <DialogPrimitive.Overlay class="fixed inset-0 z-50 bg-black/30 backdrop-blur-sm" />
  <DialogPrimitive.Content
    {...restProps}
    class={cn(
      'fixed left-1/2 top-1/2 z-50 w-[min(32rem,calc(100vw-2rem))] max-h-[min(85vh,42rem)] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-box border border-base-300 bg-base-100 shadow-2xl',
      className
    )}
  >
    {@render children?.()}
  </DialogPrimitive.Content>
</DialogPrimitive.Portal>
