<script lang="ts">
  interface Span {
    text: string;
    colorIndex?: number;
    markedBy?: string;
  }

  let {
    label,
    spans
  }: {
    label?: string;
    spans: Span[];
  } = $props();

  const HIGHLIGHT_COLORS = [
    'bg-[var(--h-amber)]',
    'bg-[var(--h-sage)]',
    'bg-[var(--h-rose)]',
    'bg-[var(--h-blue)]',
    'bg-[var(--h-lilac)]',
    'bg-[var(--h-amber-l)]'
  ];

  function highlightBg(colorIndex?: number): string {
    if (!colorIndex) return '';
    return HIGHLIGHT_COLORS[((colorIndex - 1) % 6 + 6) % 6];
  }
</script>

<div class="px-8 pb-9 pt-8 max-md:px-5 max-md:pb-7 max-md:pt-6">
  {#if label}
    <div class="mb-3 flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.14em] text-base-content/60">
      <span class="text-primary">⎙</span> {label}
    </div>
  {/if}

  <p class="m-0 mb-7 max-w-[62ch] font-serif text-[22px] leading-[1.65] text-base-content">
    {#each spans as span, i (i)}
      {#if span.colorIndex}
        <span
          class="relative -mx-0.5 cursor-pointer rounded-sm px-1 py-0.5 after:pointer-events-none after:absolute after:bottom-[calc(100%+6px)] after:left-1/2 after:-translate-x-1/2 after:whitespace-nowrap after:rounded-sm after:border after:border-base-300 after:bg-white after:px-2 after:py-0.5 after:font-mono after:text-[9px] after:uppercase after:tracking-[0.1em] after:text-base-content after:opacity-0 after:transition-opacity after:content-[attr(data-by)] hover:after:opacity-100 {highlightBg(span.colorIndex)}"
          data-by={span.markedBy ?? ''}
        >{span.text}</span>
      {:else}
        {span.text}
      {/if}
    {/each}
  </p>
</div>
