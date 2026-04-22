<script lang="ts">
  import { Dialog as DialogPrimitive, type DialogDescriptionProps } from 'bits-ui';
  import { cn } from '$lib/ndk/utils/cn';

  let { class: className = '', children, ...restProps }: DialogDescriptionProps = $props();
</script>

<DialogPrimitive.Description {...restProps} class={cn('m-0 text-sm opacity-70', className)}>
  {@render children?.()}
</DialogPrimitive.Description>
