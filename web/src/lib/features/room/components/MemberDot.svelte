<script lang="ts">
  type Size = 'sm' | 'md' | 'lg';

  let {
    colorIndex,
    size = 'md'
  }: {
    colorIndex: number;
    size?: Size;
  } = $props();

  const TINTS = [
    'var(--h-amber)',
    'var(--h-sage)',
    'var(--h-blue)',
    'var(--h-rose)',
    'var(--h-lilac)',
    'var(--h-amber-l)'
  ] as const;

  const SIZES: Record<Size, string> = {
    sm: '24px',
    md: '36px',
    lg: '48px'
  };

  const tint = $derived(TINTS[((colorIndex - 1) % 6 + 6) % 6]);
  const diameter = $derived(SIZES[size]);
</script>

<!--
  Accessibility note: When used purely decoratively (e.g. inside MemberStack),
  the parent should supply aria-hidden="true" on the stack container.
  When used as an avatar with meaningful identity, pass aria-label to the parent
  wrapper or add role="img" + aria-label here. Not critical for M1 decoration use.
-->
<div
  class="member-dot"
  style:width={diameter}
  style:height={diameter}
  style:background-color={tint}
></div>

<style>
  .member-dot {
    border-radius: 50%;
    border: 2.5px solid var(--bg);
    flex-shrink: 0;
  }
</style>
