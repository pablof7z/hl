<script lang="ts">
  type SizePreset = 'sm' | 'md' | 'lg';

  let {
    colorIndex,
    size = 'md',
    initials,
    title,
    online = false
  }: {
    colorIndex: number;
    size?: SizePreset | number;
    initials?: string;
    title?: string;
    online?: boolean;
  } = $props();

  const TINTS = [
    'var(--h-amber)',
    'var(--h-sage)',
    'var(--h-blue)',
    'var(--h-rose)',
    'var(--h-lilac)',
    'var(--h-amber-l)'
  ] as const;

  const PRESET_PX: Record<SizePreset, number> = {
    sm: 24,
    md: 36,
    lg: 48
  };

  const tint = $derived(TINTS[((colorIndex - 1) % 6 + 6) % 6]);
  const px = $derived(typeof size === 'number' ? size : PRESET_PX[size]);
  const fontSize = $derived(Math.max(9, Math.round(px * 0.32)));
</script>

<span
  class="member-dot"
  class:has-initials={!!initials}
  class:online
  style:width="{px}px"
  style:height="{px}px"
  style:line-height="{px}px"
  style:font-size="{fontSize}px"
  style:background-color={tint}
  {title}
  role={initials ? 'img' : undefined}
  aria-label={title}
>
  {#if initials}{initials}{/if}
</span>

<style>
  .member-dot {
    display: inline-block;
    border-radius: 50%;
    flex-shrink: 0;
    font-family: var(--font-sans);
    font-weight: 600;
    text-align: center;
    color: var(--ink);
    position: relative;
    vertical-align: middle;
  }

  .member-dot.online::after {
    content: '';
    position: absolute;
    bottom: 0;
    right: 0;
    width: 28%;
    height: 28%;
    min-width: 8px;
    min-height: 8px;
    border-radius: 50%;
    background: #7CAE7A;
    border: 2px solid var(--surface);
  }
</style>
