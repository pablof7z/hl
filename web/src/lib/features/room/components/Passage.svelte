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

  const MEMBER_CLASSES = [
    'hl-1',
    'hl-2',
    'hl-3',
    'hl-4',
    'hl-5',
    'hl-6'
  ];

  function memberClass(colorIndex?: number): string {
    if (!colorIndex) return '';
    return MEMBER_CLASSES[((colorIndex - 1) % 6 + 6) % 6];
  }
</script>

<div class="passage-wrap">
  {#if label}
    <div class="passage-label"><span class="pin-ico">⎙</span> {label}</div>
  {/if}

  <p class="passage">
    {#each spans as span, i (i)}
      {#if span.colorIndex}
        <span
          class="hl {memberClass(span.colorIndex)}"
          data-by={span.markedBy ?? ''}
        >{span.text}</span>
      {:else}
        {span.text}
      {/if}
    {/each}
  </p>
</div>

<style>
  .passage-wrap {
    padding: 32px 32px 36px;
  }

  @media (max-width: 760px) {
    .passage-wrap {
      padding: 24px 20px 28px;
    }
  }

  .passage-label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-fade);
    margin-bottom: 12px;
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .pin-ico {
    color: var(--brand-accent);
  }

  .passage {
    font-family: var(--font-serif);
    font-size: 22px;
    line-height: 1.65;
    color: var(--ink);
    margin: 0 0 28px;
    max-width: 62ch;
  }

  .passage :global(.hl) {
    padding: 2px 4px;
    margin: 0 -2px;
    border-radius: 2px;
    position: relative;
    cursor: pointer;
  }

  .passage :global(.hl::after) {
    content: attr(data-by);
    position: absolute;
    bottom: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink);
    white-space: nowrap;
    background: var(--surface);
    padding: 3px 8px;
    border: 1px solid var(--rule);
    border-radius: 2px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 150ms ease;
  }

  .passage :global(.hl:hover::after) {
    opacity: 1;
  }

  .passage :global(.hl-1) { background: var(--h-amber); }
  .passage :global(.hl-2) { background: var(--h-sage); }
  .passage :global(.hl-3) { background: var(--h-rose); }
  .passage :global(.hl-4) { background: var(--h-blue); }
  .passage :global(.hl-5) { background: var(--h-lilac); }
  .passage :global(.hl-6) { background: var(--h-amber-l); }
</style>
