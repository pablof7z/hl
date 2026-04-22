<script lang="ts">
  import HighlightEntry from './HighlightEntry.svelte';
  import SeeAllLink from './SeeAllLink.svelte';

  interface HighlightRow {
    id: string;
    authorPubkey: string;
    colorIndex: number;
    quote: string;
    location?: string;
    date?: string;
    replies?: number;
  }

  interface MemberFilter {
    pubkey: string;
    colorIndex: number;
    count: number;
  }

  let {
    highlights,
    totalCount,
    memberFilters = [],
    seeAllHref = '#'
  }: {
    highlights: HighlightRow[];
    totalCount?: number;
    memberFilters?: MemberFilter[];
    seeAllHref?: string;
  } = $props();

  let activePubkey = $state<string>('all');

  const filtered = $derived(
    activePubkey === 'all'
      ? highlights
      : highlights.filter((h) => h.authorPubkey === activePubkey)
  );

  const total = $derived(totalCount ?? highlights.length);

  const DOT_COLORS: Record<number, string> = {
    1: 'bg-[var(--h-amber)]',
    2: 'bg-[var(--h-sage)]',
    3: 'bg-[var(--h-blue)]',
    4: 'bg-[var(--h-rose)]',
    5: 'bg-[var(--h-lilac)]',
    6: 'bg-[var(--h-amber-l)]'
  };

  function dotClass(colorIndex: number): string {
    return DOT_COLORS[((colorIndex - 1) % 6 + 6) % 6 + 1] ?? '';
  }
</script>

<div class="flex flex-wrap items-center justify-between gap-3 border-b border-base-300 px-8 pb-3.5 pt-4 max-md:px-5 max-md:py-3.5">
  <div class="flex flex-wrap gap-2">
    {#each [{ key: 'all', label: 'All', count: total, color: '' }, ...memberFilters.map((mf) => ({ key: mf.pubkey, label: mf.pubkey.slice(0, 4), count: mf.count, color: dotClass(mf.colorIndex), aria: `Filter by ${mf.pubkey.slice(0, 8)}` }))] as f (f.key)}
      {@const isOn = activePubkey === f.key}
      <button
        type="button"
        class="inline-flex cursor-pointer items-center gap-1.5 rounded-full border border-base-300 bg-base-100 px-3 py-1.5 text-xs font-medium text-base-content/80 transition-all hover:border-base-content hover:text-base-content"
        class:!bg-base-content={isOn}
        class:!text-base-100={isOn}
        class:!border-base-content={isOn}
        onclick={() => (activePubkey = f.key)}
        aria-label={'aria' in f ? f.aria : undefined}
      >
        {#if f.color}
          <span class="size-2.5 rounded-full {f.color}"></span>
        {/if}
        {f.label}
        <span class="font-mono text-[10px] font-normal opacity-70">{f.count}</span>
      </button>
    {/each}
  </div>
  <div class="text-xs font-medium text-base-content/60">By position ↓</div>
</div>

<div class="px-8 max-md:px-5">
  {#if filtered.length === 0}
    <p class="m-0 py-10 text-center text-[15px] text-base-content/60">No highlights yet.</p>
  {:else}
    {#each filtered as hl (hl.id)}
      <HighlightEntry
        id={hl.id}
        authorPubkey={hl.authorPubkey}
        colorIndex={hl.colorIndex}
        quote={hl.quote}
        location={hl.location}
        date={hl.date}
        replies={hl.replies}
      />
    {/each}
  {/if}
</div>

<div class="px-8 pb-7 max-md:px-5 max-md:pb-6">
  <SeeAllLink label="See all {total} highlights" href={seeAllHref} />
</div>
