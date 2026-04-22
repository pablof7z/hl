<script lang="ts">
  import { Dialog as DialogPrimitive, type DialogTitleProps } from 'bits-ui';
  import { cn } from '$lib/ndk/utils/cn';

  let { class: className = '', children, ...restProps }: DialogTitleProps = $props();
</script>

<DialogPrimitive.Title
  {...restProps}
  class={cn('m-0 font-serif text-2xl font-bold leading-tight tracking-tight text-base-content', className)}
>
  {@render children?.()}
</DialogPrimitive.Title>
