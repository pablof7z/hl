<script lang="ts">
  import type { CommunitySummary } from '$lib/ndk/groups';
  import RoomCard from '$lib/features/groups/RoomCard.svelte';

  type SortMode = 'featured' | 'name' | 'newest';
  type AccessFilter = 'all' | 'open' | 'closed';
  type VisibilityFilter = 'all' | 'public' | 'private';

  let {
    communities,
    joinedGroupIds = [],
    emptyLabel = 'No communities found.',
    emptyCopy = '',
    emptyCtaHref = '',
    emptyCtaLabel = '',
    searchPlaceholder = 'Search communities',
    defaultSort = 'featured',
    showVisibilityFilter = true
  }: {
    communities: CommunitySummary[];
    joinedGroupIds?: string[];
    emptyLabel?: string;
    emptyCopy?: string;
    emptyCtaHref?: string;
    emptyCtaLabel?: string;
    searchPlaceholder?: string;
    defaultSort?: SortMode;
    showVisibilityFilter?: boolean;
  } = $props();

  let query = $state('');
  let accessFilter = $state<AccessFilter>('all');
  let visibilityFilter = $state<VisibilityFilter>('all');
  let sortMode = $state<SortMode>('featured');

  $effect(() => {
    sortMode = defaultSort;
  });

  const joinedSet = $derived(new Set(joinedGroupIds));
  const showAccessControl = $derived(
    communities.some((community) => community.access === 'open') &&
      communities.some((community) => community.access === 'closed')
  );
  const showVisibilityControl = $derived(
    showVisibilityFilter &&
      communities.some((community) => community.visibility === 'private') &&
      communities.some((community) => community.visibility === 'public')
  );
  const filteredCommunities = $derived.by(() => {
    const normalizedQuery = query.trim().toLowerCase();

    return communities
      .filter((community) => {
        if (accessFilter !== 'all' && community.access !== accessFilter) {
          return false;
        }

        if (showVisibilityControl && visibilityFilter !== 'all' && community.visibility !== visibilityFilter) {
          return false;
        }

        if (!normalizedQuery) {
          return true;
        }

        const haystack = `${room.name} ${community.about} ${room.id}`.toLowerCase();
        return haystack.includes(normalizedQuery);
      })
      .toSorted((left, right) => {
        if (sortMode === 'name') {
          return left.name.localeCompare(right.name);
        }

        if (sortMode === 'newest') {
          return (right.createdAt ?? 0) - (left.createdAt ?? 0);
        }

        const leftJoined = joinedSet.has(left.id) ? 1 : 0;
        const rightJoined = joinedSet.has(right.id) ? 1 : 0;
        if (rightJoined !== leftJoined) {
          return rightJoined - leftJoined;
        }

        const leftMembers = left.memberCount ?? -1;
        const rightMembers = right.memberCount ?? -1;
        if (rightMembers !== leftMembers) {
          return rightMembers - leftMembers;
        }

        if ((right.createdAt ?? 0) !== (left.createdAt ?? 0)) {
          return (right.createdAt ?? 0) - (left.createdAt ?? 0);
        }

        return left.name.localeCompare(right.name);
      });
  });
</script>

<section class="community-browser">
  <div class="community-browser-toolbar">
    <label class="search-field">
      <span>Search</span>
      <input bind:value={query} type="search" placeholder={searchPlaceholder} />
    </label>

    <div class="browser-controls">
      {#if showAccessControl}
        <label class="select-field">
          <span>Access</span>
          <select bind:value={accessFilter}>
            <option value="all">All</option>
            <option value="open">Open</option>
            <option value="closed">Closed</option>
          </select>
        </label>
      {/if}

      {#if showVisibilityControl}
        <label class="select-field">
          <span>Visibility</span>
          <select bind:value={visibilityFilter}>
            <option value="all">All</option>
            <option value="public">Public</option>
            <option value="private">Private</option>
          </select>
        </label>
      {/if}

      <label class="select-field">
        <span>Sort</span>
        <select bind:value={sortMode}>
          <option value="featured">Featured</option>
          <option value="newest">Newest</option>
          <option value="name">A-Z</option>
        </select>
      </label>
    </div>
  </div>

  <p class="result-label">
    Showing {filteredCommunities.length} of {communities.length} circle{communities.length === 1 ? '' : 's'}
  </p>

  {#if filteredCommunities.length === 0}
    <section class="empty-state">
      <p class="empty-label">{emptyLabel}</p>
      {#if emptyCopy}
        <p class="empty-copy">{emptyCopy}</p>
      {/if}
      {#if emptyCtaHref && emptyCtaLabel}
        <a class="empty-cta" href={emptyCtaHref}>{emptyCtaLabel}</a>
      {/if}
    </section>
  {:else}
    <div class="community-grid">
      {#each filteredCommunities as community (community.id)}
        <RoomCard community={community} joined={joinedSet.has(community.id)} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .community-browser {
    display: grid;
    gap: 1rem;
  }

  .community-browser-toolbar {
    display: grid;
    gap: 0.9rem;
  }

  .search-field,
  .select-field {
    display: grid;
    gap: 0.35rem;
  }

  .search-field span,
  .select-field span,
  .result-label {
    margin: 0;
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .search-field input,
  .select-field select {
    width: 100%;
    min-height: 2.85rem;
    padding: 0 0.85rem;
    border: 1px solid var(--border);
    border-radius: 0.9rem;
    background: var(--surface);
    color: var(--text);
    font: inherit;
  }

  .browser-controls {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: 0.75rem;
  }

  .result-label {
    letter-spacing: 0.04em;
  }

  .empty-state {
    display: grid;
    gap: 0.75rem;
    max-width: 42rem;
    padding: 1.75rem;
    border: 1px solid var(--border);
    border-radius: 1.4rem;
    background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 6%, transparent), transparent);
  }

  .empty-label {
    margin: 0;
    color: var(--text-strong);
    font-size: 1.1rem;
    font-weight: 700;
  }

  .empty-copy {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .empty-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: fit-content;
    min-height: 2.9rem;
    padding: 0 1rem;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 600;
    transition: background 120ms ease;
  }

  .empty-cta:hover {
    background: var(--accent-hover);
  }

  .community-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1rem;
  }

  @media (min-width: 760px) {
    .community-browser-toolbar {
      grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
      align-items: end;
    }
  }
</style>
