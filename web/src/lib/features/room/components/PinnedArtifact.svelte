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
    colorIndex: number;
    initials: string;
    name?: string;
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
    colorIndex: number;
    initials: string;
    name: string;
    handle: string;
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
    stats,
    readers,
    readersNote,
    tabCounts,
    passageLabel,
    passageSpans,
    threadTitle,
    threadStarter,
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
    stats?: Stat[];
    readers?: Reader[];
    readersNote?: string;
    tabCounts: TabCounts;
    passageLabel?: string;
    passageSpans: Span[];
    threadTitle?: string;
    threadStarter?: string;
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

<div class="pinned">
  <PinnedHeader
    {title}
    {subtitle}
    {coverTitle}
    {coverAuthor}
    {coverKicker}
    {coverVariant}
    {stats}
    {readers}
    {readersNote}
  />

  <div class="pin-tabs" role="tablist" aria-label="Pinned artifact sections">
    {#each PINNED_TABS as tab (tab)}
      <button
        type="button"
        id={`pin-tab-${tab.toLowerCase()}`}
        class="pin-tab"
        class:active={activeTab === tab}
        role="tab"
        aria-selected={activeTab === tab}
        aria-controls={`pin-panel-${tab.toLowerCase()}`}
        tabindex={activeTab === tab ? 0 : -1}
        onclick={() => (activeTab = tab)}
        onkeydown={handleTabKeydown}
      >
        {tab}
        <span class="count">{countFor[tab]}</span>
      </button>
    {/each}
  </div>

  <div
    id="pin-panel-discussions"
    class="pin-panel"
    class:active={activeTab === 'Discussions'}
    role="tabpanel"
    aria-labelledby="pin-tab-discussions"
    hidden={activeTab !== 'Discussions'}
  >
    <DiscussionsTab
      {passageLabel}
      {passageSpans}
      {threadTitle}
      {threadStarter}
      {threadStartedAt}
      {messages}
    />
  </div>

  <div
    id="pin-panel-highlights"
    class="pin-panel"
    class:active={activeTab === 'Highlights'}
    role="tabpanel"
    aria-labelledby="pin-tab-highlights"
    hidden={activeTab !== 'Highlights'}
  >
    <HighlightsTab
      {highlights}
      totalCount={tabCounts.highlights}
      memberFilters={memberFilters ?? []}
    />
  </div>

  <div
    id="pin-panel-notes"
    class="pin-panel"
    class:active={activeTab === 'Notes'}
    role="tabpanel"
    aria-labelledby="pin-tab-notes"
    hidden={activeTab !== 'Notes'}
  >
    <NotesTab {notes} />
  </div>

  <div
    id="pin-panel-members"
    class="pin-panel"
    class:active={activeTab === 'Members'}
    role="tabpanel"
    aria-labelledby="pin-tab-members"
    hidden={activeTab !== 'Members'}
  >
    <MembersTable members={membersTableRows} />
  </div>
</div>

<style>
  .pinned {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    box-shadow: 0 18px 40px -22px rgba(21, 19, 15, 0.12);
    overflow: hidden;
  }

  .pin-tabs {
    display: flex;
    border-bottom: 1px solid var(--rule);
    padding: 0 32px;
    gap: 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .pin-tabs::-webkit-scrollbar {
    display: none;
  }

  @media (max-width: 760px) {
    .pin-tabs {
      padding: 0 20px;
    }
  }

  .pin-tab {
    padding: 14px 20px 12px;
    font-size: 13px;
    font-family: var(--font-sans);
    font-weight: 500;
    color: var(--ink-fade);
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    white-space: nowrap;
    transition: color 150ms ease;
  }

  .pin-tab.active {
    color: var(--ink);
    border-bottom-color: var(--brand-accent);
  }

  .pin-tab:hover {
    color: var(--ink);
  }

  .pin-tab:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: -2px;
  }

  .count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-fade);
    background: var(--surface-muted);
    padding: 1px 6px;
    border-radius: 999px;
  }

  .pin-tab.active .count {
    background: var(--marker);
    color: var(--ink);
  }

  .pin-panel {
    display: none;
  }

  .pin-panel.active {
    display: block;
  }
</style>
