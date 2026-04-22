<script module lang="ts">
  export const PINNED_TABS = ['Discussions', 'Highlights', 'Notes', 'Members'] as const;
  export type PinnedTab = (typeof PINNED_TABS)[number];
</script>

<script lang="ts">
  import PinnedHeader from './PinnedHeader.svelte';
  import DiscussionsTab from './DiscussionsTab.svelte';
  import HighlightsTab from './HighlightsTab.svelte';
  import NotesTab from './NotesTab.svelte';
  import MembersTable from './MembersTable.svelte';

  interface Reader {
    pubkey: string;
    colorIndex: number;
  }

  interface Stat {
    value: string;
    label: string;
  }

  interface Span {
    text: string;
    colorIndex?: number;
    markedBy?: string;
  }

  interface Message {
    id: string;
    pubkey: string;
    colorIndex: number;
    time: string;
    body: string;
    isReply?: boolean;
  }

  interface TabCounts {
    discussions: number;
    highlights: number;
    notes: number;
    members: number;
  }

  let {
    title,
    subtitle,
    coverTitle,
    coverAuthor,
    coverKicker,
    coverVariant = 'dark',
    image,
    openHref = '#',
    continueHref = '#',
    continueLabel = 'Continue reading',
    stats,
    readers,
    readersNote,
    tabCounts,
    passageLabel,
    passageSpans,
    threadTitle,
    threadStarterPubkey,
    threadStartedAt,
    messages,
    highlights,
    memberFilters,
    notes,
    membersTableRows,
    defaultTab = 'Discussions'
  }: {
    title: string;
    subtitle?: string;
    coverTitle: string;
    coverAuthor?: string;
    coverKicker?: string;
    coverVariant?: 'dark' | 'red' | 'blue' | 'green' | 'plum';
    image?: string;
    openHref?: string;
    continueHref?: string;
    continueLabel?: string;
    stats?: Stat[];
    readers?: Reader[];
    readersNote?: string;
    tabCounts: TabCounts;
    passageLabel?: string;
    passageSpans: Span[];
    threadTitle?: string;
    threadStarterPubkey?: string;
    threadStartedAt?: string;
    messages: Message[];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    highlights: any[];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    memberFilters?: any[];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    notes: any[];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    membersTableRows: any[];
    defaultTab?: PinnedTab;
  } = $props();

  let activeTab = $state<PinnedTab>('Discussions');
  $effect(() => {
    activeTab = defaultTab;
  });

  function handleTabKeydown(event: KeyboardEvent) {
    const i = PINNED_TABS.indexOf(activeTab);
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      activeTab = PINNED_TABS[(i + 1) % PINNED_TABS.length];
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      activeTab = PINNED_TABS[(i - 1 + PINNED_TABS.length) % PINNED_TABS.length];
    } else if (event.key === 'Home') {
      event.preventDefault();
      activeTab = PINNED_TABS[0];
    } else if (event.key === 'End') {
      event.preventDefault();
      activeTab = PINNED_TABS[PINNED_TABS.length - 1];
    }
  }

  const countFor: Record<PinnedTab, number> = $derived({
    Discussions: tabCounts.discussions,
    Highlights: tabCounts.highlights,
    Notes: tabCounts.notes,
    Members: tabCounts.members
  });
</script>

<div class="overflow-hidden rounded border border-base-300 bg-base-100 shadow-[0_18px_40px_-22px_rgba(21,19,15,0.12)]">
  <PinnedHeader
    {title}
    {subtitle}
    {coverTitle}
    {coverAuthor}
    {coverKicker}
    {coverVariant}
    {image}
    {openHref}
    {continueHref}
    {continueLabel}
    {stats}
    {readers}
    {readersNote}
  />

  <div
    class="flex gap-1 overflow-x-auto border-b border-base-300 px-8 max-md:px-5 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    role="tablist"
    aria-label="Pinned artifact sections"
  >
    {#each PINNED_TABS as tab (tab)}
      {@const isActive = activeTab === tab}
      <button
        type="button"
        id={`pin-tab-${tab.toLowerCase()}`}
        class="flex cursor-pointer items-center gap-2 whitespace-nowrap border-0 border-b-2 border-transparent bg-transparent px-5 pb-3 pt-3.5 text-[13px] font-medium text-base-content/60 transition-colors hover:text-base-content focus-visible:outline focus-visible:-outline-offset-2 focus-visible:outline-primary"
        class:!border-primary={isActive}
        class:!text-base-content={isActive}
        role="tab"
        aria-selected={isActive}
        aria-controls={`pin-panel-${tab.toLowerCase()}`}
        tabindex={isActive ? 0 : -1}
        onclick={() => (activeTab = tab)}
        onkeydown={handleTabKeydown}
      >
        {tab}
        <span
          class="rounded-full bg-base-200 px-1.5 py-px font-mono text-[11px] text-base-content/60"
          class:!bg-accent={isActive}
          class:!text-base-content={isActive}
        >{countFor[tab]}</span>
      </button>
    {/each}
  </div>

  <div
    id="pin-panel-discussions"
    role="tabpanel"
    aria-labelledby="pin-tab-discussions"
    hidden={activeTab !== 'Discussions'}
    class:hidden={activeTab !== 'Discussions'}
  >
    <DiscussionsTab
      {passageLabel}
      {passageSpans}
      {threadTitle}
      {threadStarterPubkey}
      {threadStartedAt}
      {messages}
    />
  </div>

  <div
    id="pin-panel-highlights"
    role="tabpanel"
    aria-labelledby="pin-tab-highlights"
    hidden={activeTab !== 'Highlights'}
    class:hidden={activeTab !== 'Highlights'}
  >
    <HighlightsTab
      {highlights}
      totalCount={tabCounts.highlights}
      memberFilters={memberFilters ?? []}
    />
  </div>

  <div
    id="pin-panel-notes"
    role="tabpanel"
    aria-labelledby="pin-tab-notes"
    hidden={activeTab !== 'Notes'}
    class:hidden={activeTab !== 'Notes'}
  >
    <NotesTab {notes} />
  </div>

  <div
    id="pin-panel-members"
    role="tabpanel"
    aria-labelledby="pin-tab-members"
    hidden={activeTab !== 'Members'}
    class:hidden={activeTab !== 'Members'}
  >
    <MembersTable members={membersTableRows} />
  </div>
</div>
