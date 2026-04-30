<script lang="ts">
  import { ndk, ensureClientNdk } from '$lib/ndk/client';
  import { nip19 } from '@nostr-dev-kit/ndk';
  import {
    parseKindleClippings,
    type ParsedClipping,
    type ParseSummary
  } from '$lib/features/import/kindleClippings';
  import {
    searchOpenLibrary,
    type OpenLibraryMatch
  } from '$lib/features/import/openlibrarySearch';
  import {
    publishKindleHighlights,
    type PublishStatus,
    type ResolvedClipping
  } from '$lib/features/import/publishKindleHighlights';

  // ─── Stages ─────────────────────────────────────────────────────────────────
  type Stage = 'drop' | 'review' | 'publish';

  let stage = $state<Stage>('drop');
  let parserError = $state('');
  let dragOver = $state(false);
  let pasteText = $state('');

  // ─── Parsed data ────────────────────────────────────────────────────────────
  let summary = $state<ParseSummary | null>(null);
  let entries = $state<ParsedClipping[]>([]);
  let selected = $state<Set<string>>(new Set());

  // Per-book ISBN lookup state (key = `title|author`).
  type LookupState = {
    status: 'pending' | 'looking' | 'matched' | 'no-match';
    match: OpenLibraryMatch | null;
  };
  let lookups = $state<Map<string, LookupState>>(new Map());

  // Collapsed book sections.
  let collapsedBooks = $state<Set<string>>(new Set());

  // ─── Publish state ──────────────────────────────────────────────────────────
  let publishStatuses = $state<Map<string, PublishStatus>>(new Map());
  let publishing = $state(false);
  let publishComplete = $state(false);
  let abortController = $state<AbortController | null>(null);

  // ─── NDK / auth ─────────────────────────────────────────────────────────────
  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  // ─── Derived: book grouping ─────────────────────────────────────────────────
  type BookGroup = {
    key: string;
    title: string;
    author?: string;
    entries: ParsedClipping[];
  };

  function bookKey(title: string, author?: string): string {
    return `${title}|${author ?? ''}`;
  }

  const bookGroups = $derived.by<BookGroup[]>(() => {
    const groups = new Map<string, BookGroup>();
    for (const entry of entries) {
      const key = bookKey(entry.title, entry.author);
      const existing = groups.get(key);
      if (existing) {
        existing.entries.push(entry);
      } else {
        groups.set(key, {
          key,
          title: entry.title,
          author: entry.author,
          entries: [entry]
        });
      }
    }
    return [...groups.values()];
  });

  const totalSelected = $derived(selected.size);
  const totalEntries = $derived(entries.length);

  // ─── Drop-zone handlers ─────────────────────────────────────────────────────

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    dragOver = true;
  }
  function handleDragLeave(event: DragEvent) {
    event.preventDefault();
    dragOver = false;
  }
  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragOver = false;
    const file = event.dataTransfer?.files?.[0];
    if (!file) return;
    await ingestFile(file);
  }

  async function handleFileInput(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    await ingestFile(file);
    input.value = '';
  }

  async function ingestFile(file: File) {
    parserError = '';
    try {
      const text = await file.text();
      ingestText(text);
    } catch (err) {
      parserError = err instanceof Error ? err.message : 'Could not read file.';
    }
  }

  function ingestPaste() {
    parserError = '';
    if (!pasteText.trim()) {
      parserError = 'Paste your clippings text first.';
      return;
    }
    ingestText(pasteText);
  }

  function ingestText(text: string) {
    const result = parseKindleClippings(text);
    if (result.entries.length === 0) {
      parserError = `Parsed 0 highlights (skipped ${result.skipped.notes} notes, ${result.skipped.bookmarks} bookmarks, ${result.skipped.malformed} malformed).`;
      return;
    }
    summary = result;
    entries = result.entries;
    selected = new Set(result.entries.map((e) => e.id));
    collapsedBooks = new Set();
    stage = 'review';

    // Kick off ISBN lookups in the background; UI updates progressively.
    void resolveAllLookups();
  }

  // ─── ISBN lookup orchestration ──────────────────────────────────────────────

  async function resolveAllLookups() {
    const next = new Map<string, LookupState>();
    for (const group of bookGroups) {
      next.set(group.key, { status: 'pending', match: null });
    }
    lookups = next;

    for (const group of bookGroups) {
      // Mark looking up as we begin each book.
      const updating = new Map(lookups);
      updating.set(group.key, { status: 'looking', match: null });
      lookups = updating;

      const match = await searchOpenLibrary(group.title, group.author);
      const after = new Map(lookups);
      after.set(group.key, {
        status: match ? 'matched' : 'no-match',
        match
      });
      lookups = after;
    }
  }

  // ─── Selection helpers ──────────────────────────────────────────────────────

  function toggleEntry(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  function toggleBook(group: BookGroup) {
    const allSelected = group.entries.every((e) => selected.has(e.id));
    const next = new Set(selected);
    if (allSelected) {
      for (const e of group.entries) next.delete(e.id);
    } else {
      for (const e of group.entries) next.add(e.id);
    }
    selected = next;
  }

  function selectAll() {
    selected = new Set(entries.map((e) => e.id));
  }
  function selectNone() {
    selected = new Set();
  }

  function toggleCollapse(key: string) {
    const next = new Set(collapsedBooks);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsedBooks = next;
  }

  // ─── Publish flow ──────────────────────────────────────────────────────────

  function buildResolvedClippings(): ResolvedClipping[] {
    const list: ResolvedClipping[] = [];
    for (const entry of entries) {
      if (!selected.has(entry.id)) continue;
      const key = bookKey(entry.title, entry.author);
      const lookup = lookups.get(key);
      list.push({
        parsed: entry,
        match: lookup?.match ?? null
      });
    }
    return list;
  }

  async function startPublish() {
    if (!currentUser) {
      parserError = 'Sign in before publishing.';
      return;
    }
    if (isReadOnly) {
      parserError = 'This signer is read-only — switch to a writable signer.';
      return;
    }
    if (totalSelected === 0) {
      parserError = 'Select at least one highlight.';
      return;
    }

    parserError = '';
    publishStatuses = new Map();
    publishComplete = false;
    publishing = true;
    stage = 'publish';
    abortController = new AbortController();

    try {
      await ensureClientNdk();
      const items = buildResolvedClippings();

      for await (const status of publishKindleHighlights(ndk, items, {
        rateLimit: 5,
        signal: abortController.signal
      })) {
        const next = new Map(publishStatuses);
        next.set(status.id, status);
        publishStatuses = next;
      }
    } catch (err) {
      parserError = err instanceof Error ? err.message : 'Publishing failed.';
    } finally {
      publishing = false;
      publishComplete = true;
    }
  }

  async function retryFailed() {
    const failedIds = [...publishStatuses.entries()]
      .filter(([, status]) => status.state === 'failed')
      .map(([id]) => id);
    if (failedIds.length === 0) return;

    const failedEntries = entries.filter((e) => failedIds.includes(e.id));
    const items: ResolvedClipping[] = failedEntries.map((entry) => {
      const key = bookKey(entry.title, entry.author);
      const lookup = lookups.get(key);
      return { parsed: entry, match: lookup?.match ?? null };
    });

    publishComplete = false;
    publishing = true;
    abortController = new AbortController();

    try {
      for await (const status of publishKindleHighlights(ndk, items, {
        rateLimit: 5,
        signal: abortController.signal
      })) {
        const next = new Map(publishStatuses);
        next.set(status.id, status);
        publishStatuses = next;
      }
    } finally {
      publishing = false;
      publishComplete = true;
    }
  }

  function cancelPublish() {
    abortController?.abort();
  }

  function reset() {
    stage = 'drop';
    parserError = '';
    pasteText = '';
    summary = null;
    entries = [];
    selected = new Set();
    lookups = new Map();
    collapsedBooks = new Set();
    publishStatuses = new Map();
    publishing = false;
    publishComplete = false;
    abortController = null;
  }

  // ─── Publish status counters ───────────────────────────────────────────────

  const publishCounts = $derived.by(() => {
    let queued = 0;
    let working = 0;
    let done = 0;
    let failed = 0;
    for (const status of publishStatuses.values()) {
      switch (status.state) {
        case 'queued':
          queued += 1;
          break;
        case 'publishing':
          working += 1;
          break;
        case 'published':
          done += 1;
          break;
        case 'failed':
          failed += 1;
          break;
      }
    }
    return { queued, working, done, failed };
  });

  const totalToPublish = $derived(publishStatuses.size);
  const publishProgress = $derived(
    totalToPublish === 0
      ? 0
      : Math.round(((publishCounts.done + publishCounts.failed) / totalToPublish) * 100)
  );

  // ─── User profile link for the success screen ─────────────────────────────
  const profileNpub = $derived.by(() => {
    if (!currentUser) return '';
    try {
      return nip19.npubEncode(currentUser.pubkey);
    } catch {
      return '';
    }
  });

  // ─── Helpers for templates ────────────────────────────────────────────────
  function lookupLabel(state: LookupState | undefined): string {
    if (!state || state.status === 'pending') return 'Queued for ISBN lookup';
    if (state.status === 'looking') return 'Looking up…';
    if (state.status === 'matched' && state.match?.isbn13) {
      return `Matched ISBN ${state.match.isbn13}`;
    }
    if (state.status === 'matched') return 'Matched (no ISBN)';
    return 'No ISBN match';
  }

  function lookupBadgeClass(state: LookupState | undefined): string {
    if (!state || state.status === 'pending' || state.status === 'looking') {
      return 'badge-ghost';
    }
    if (state.status === 'matched' && state.match?.isbn13) return 'badge-success';
    if (state.status === 'matched') return 'badge-warning';
    return 'badge-warning';
  }

  function formatAddedAt(date: Date): string {
    if (!date || Number.isNaN(date.getTime()) || date.getTime() === 0) return '';
    return date.toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric'
    });
  }

  function statusFor(id: string): PublishStatus | undefined {
    return publishStatuses.get(id);
  }
</script>

<svelte:head>
  <title>Import Kindle Clippings — Highlighter</title>
</svelte:head>

<section class="grid gap-8 max-w-[64rem] mx-auto py-10 pb-20 px-4">
  <header class="grid gap-1">
    <p class="m-0 text-xs font-semibold uppercase tracking-wider text-primary/80">Import</p>
    <h1 class="m-0 text-base-content text-3xl font-bold tracking-tight">
      Bring your Kindle highlights to Nostr
    </h1>
    <p class="m-0 text-base-content/60 text-sm max-w-prose leading-relaxed">
      Drop your <code class="bg-base-200 rounded px-1 py-0.5 text-xs">My Clippings.txt</code>
      file (or paste its contents) and we'll publish each highlight as a signed
      kind:9802 event — with the matching ISBN attached when we can find it.
    </p>
  </header>

  {#if parserError}
    <div class="alert alert-error text-sm py-2">
      <span>{parserError}</span>
    </div>
  {/if}

  <!-- ─── Stage 1: drop / paste ─── -->
  {#if stage === 'drop'}
    <div class="grid gap-6">
      <div
        role="region"
        aria-label="Drop Kindle clippings file"
        class="card border-2 border-dashed transition-colors {dragOver ? 'border-primary bg-primary/5' : 'border-base-300 bg-base-100'}"
        ondragover={handleDragOver}
        ondragleave={handleDragLeave}
        ondrop={handleDrop}
      >
        <div class="card-body items-center text-center gap-3 py-12">
          <div class="size-12 text-base-content/40">
            <svg class="size-full" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
              <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
            </svg>
          </div>
          <p class="m-0 text-base-content font-semibold text-base">Drop your <code>My Clippings.txt</code> here</p>
          <p class="m-0 text-base-content/50 text-sm">or click to choose a file</p>
          <label class="btn btn-primary btn-sm mt-2 cursor-pointer">
            Choose file
            <input
              type="file"
              accept=".txt,text/plain"
              class="hidden"
              onchange={handleFileInput}
            />
          </label>
        </div>
      </div>

      <div class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3">
          <h2 class="m-0 text-base-content font-semibold text-base">Or paste contents</h2>
          <textarea
            class="textarea textarea-bordered w-full font-mono text-xs leading-relaxed"
            rows="10"
            placeholder={`Title (Author)\n- Your Highlight on Location 234-237 | Added on …\n\nThe quote text…\n==========`}
            bind:value={pasteText}
          ></textarea>
          <div class="flex justify-end">
            <button type="button" class="btn btn-primary" onclick={ingestPaste} disabled={!pasteText.trim()}>
              Parse clippings
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- ─── Stage 2: review ─── -->
  {#if stage === 'review'}
    {#if summary}
      <div class="alert alert-info text-sm py-2">
        <div class="flex flex-wrap gap-x-4 gap-y-1">
          <span><strong>{entries.length}</strong> highlights</span>
          <span class="opacity-70">·</span>
          <span><strong>{bookGroups.length}</strong> books</span>
          {#if summary.skipped.notes > 0}
            <span class="opacity-70">·</span>
            <span>skipped {summary.skipped.notes} notes</span>
          {/if}
          {#if summary.skipped.bookmarks > 0}
            <span class="opacity-70">·</span>
            <span>skipped {summary.skipped.bookmarks} bookmarks</span>
          {/if}
          {#if summary.skipped.malformed > 0}
            <span class="opacity-70">·</span>
            <span>skipped {summary.skipped.malformed} malformed</span>
          {/if}
        </div>
      </div>
    {/if}

    <div class="flex items-center justify-between gap-3 flex-wrap sticky top-0 bg-base-200/80 backdrop-blur-sm border border-base-300 rounded-lg px-3 py-2 z-10">
      <div class="text-sm">
        <span class="font-semibold">Selected: {totalSelected}</span>
        <span class="text-base-content/50"> / {totalEntries}</span>
      </div>
      <div class="flex gap-2">
        <button type="button" class="btn btn-ghost btn-xs" onclick={selectAll}>Select all</button>
        <button type="button" class="btn btn-ghost btn-xs" onclick={selectNone}>Select none</button>
        <button type="button" class="btn btn-ghost btn-xs" onclick={reset}>Start over</button>
      </div>
    </div>

    <div class="grid gap-4">
      {#each bookGroups as group (group.key)}
        {@const lookup = lookups.get(group.key)}
        {@const collapsed = collapsedBooks.has(group.key)}
        {@const allSelected = group.entries.every((e) => selected.has(e.id))}
        {@const someSelected = group.entries.some((e) => selected.has(e.id))}
        <article class="card bg-base-100 border border-base-300 overflow-hidden">
          <header class="flex items-start gap-3 px-4 py-3 border-b border-base-300 bg-base-200/40">
            <input
              type="checkbox"
              class="checkbox checkbox-sm mt-1"
              checked={allSelected}
              indeterminate={!allSelected && someSelected}
              onchange={() => toggleBook(group)}
              aria-label="Toggle all in {group.title}"
            />
            <button
              type="button"
              class="grid gap-1 text-left flex-1 cursor-pointer"
              onclick={() => toggleCollapse(group.key)}
              aria-expanded={!collapsed}
            >
              <div class="flex items-center gap-2 flex-wrap">
                <h3 class="m-0 font-semibold text-base-content text-base leading-tight">{group.title}</h3>
                {#if group.author}
                  <span class="text-sm text-base-content/60">· {group.author}</span>
                {/if}
                <span class="badge badge-ghost badge-sm">{group.entries.length}</span>
                <span class="badge {lookupBadgeClass(lookup)} badge-sm">{lookupLabel(lookup)}</span>
              </div>
              {#if lookup?.match?.title && lookup.match.title.toLowerCase() !== group.title.toLowerCase()}
                <p class="m-0 text-xs text-base-content/50">Open Library: {lookup.match.title}{lookup.match.author ? ` — ${lookup.match.author}` : ''}</p>
              {/if}
            </button>
            <span class="text-base-content/40 text-xs select-none mt-1">{collapsed ? '▸' : '▾'}</span>
          </header>

          {#if !collapsed}
            <ul class="divide-y divide-base-300">
              {#each group.entries as entry (entry.id)}
                <li class="flex items-start gap-3 px-4 py-3">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-sm mt-1"
                    checked={selected.has(entry.id)}
                    onchange={() => toggleEntry(entry.id)}
                    aria-label="Toggle highlight"
                  />
                  <div class="grid gap-1 flex-1 min-w-0">
                    <p class="m-0 text-sm leading-relaxed text-base-content line-clamp-5">{entry.quote}</p>
                    <p class="m-0 text-xs text-base-content/50">
                      {entry.locationLabel}{formatAddedAt(entry.addedAt) ? ` · ${formatAddedAt(entry.addedAt)}` : ''}
                    </p>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
        </article>
      {/each}
    </div>

    <div class="sticky bottom-4 z-10 flex justify-center">
      <button
        type="button"
        class="btn btn-primary shadow-lg"
        onclick={startPublish}
        disabled={totalSelected === 0 || publishing || isReadOnly}
      >
        Publish {totalSelected} highlight{totalSelected === 1 ? '' : 's'}
      </button>
    </div>

    {#if isReadOnly}
      <p class="m-0 text-sm text-warning text-center">
        Your active signer is read-only. Switch to a writable signer to publish.
      </p>
    {/if}
  {/if}

  <!-- ─── Stage 3: publish ─── -->
  {#if stage === 'publish'}
    <div class="grid gap-6">
      <div class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3">
          <div class="flex items-center justify-between gap-2 flex-wrap">
            <h2 class="m-0 font-semibold text-base-content text-base">
              {publishComplete ? 'Import complete' : 'Publishing…'}
            </h2>
            {#if !publishComplete && publishing}
              <button type="button" class="btn btn-ghost btn-sm" onclick={cancelPublish}>Cancel</button>
            {/if}
          </div>
          <progress class="progress progress-primary w-full" value={publishProgress} max="100"></progress>
          <div class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-base-content/70">
            <span>queued: {publishCounts.queued}</span>
            <span>publishing: {publishCounts.working}</span>
            <span class="text-success">published: {publishCounts.done}</span>
            <span class={publishCounts.failed > 0 ? 'text-error' : ''}>failed: {publishCounts.failed}</span>
          </div>
        </div>
      </div>

      {#if publishComplete}
        <div class="alert alert-success">
          <svg class="size-5 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
          </svg>
          <div class="grid gap-1 flex-1">
            <p class="m-0 font-semibold">
              Published {publishCounts.done} highlight{publishCounts.done === 1 ? '' : 's'} from {bookGroups.length} book{bookGroups.length === 1 ? '' : 's'}
            </p>
            {#if publishCounts.failed > 0}
              <p class="m-0 text-sm opacity-80">{publishCounts.failed} failed — see below.</p>
            {/if}
          </div>
          <div class="flex gap-2 ml-auto">
            {#if publishCounts.failed > 0}
              <button type="button" class="btn btn-sm btn-warning" onclick={retryFailed}>Retry failed</button>
            {/if}
            {#if profileNpub}
              <a href="/profile/{profileNpub}" class="btn btn-sm btn-ghost">View profile</a>
            {/if}
            <button type="button" class="btn btn-sm btn-ghost" onclick={reset}>Import another file</button>
          </div>
        </div>
      {/if}

      <div class="card bg-base-100 border border-base-300">
        <div class="card-body gap-2 py-3">
          <h3 class="m-0 font-semibold text-sm text-base-content/70">Per-entry status</h3>
          <ul class="divide-y divide-base-300 max-h-[28rem] overflow-y-auto">
            {#each entries.filter((e) => publishStatuses.has(e.id)) as entry (entry.id)}
              {@const status = statusFor(entry.id)}
              <li class="flex items-start gap-3 py-2">
                <span class="mt-0.5 shrink-0">
                  {#if status?.state === 'published'}
                    <span class="badge badge-success badge-sm">published</span>
                  {:else if status?.state === 'failed'}
                    <span class="badge badge-error badge-sm">failed</span>
                  {:else if status?.state === 'publishing'}
                    <span class="badge badge-info badge-sm">publishing…</span>
                  {:else}
                    <span class="badge badge-ghost badge-sm">queued</span>
                  {/if}
                </span>
                <div class="grid gap-0.5 flex-1 min-w-0">
                  <p class="m-0 text-xs text-base-content/50 truncate">{entry.title}{entry.author ? ` · ${entry.author}` : ''}</p>
                  <p class="m-0 text-sm leading-snug text-base-content line-clamp-2">{entry.quote}</p>
                  {#if status?.state === 'failed'}
                    <p class="m-0 text-xs text-error">{status.error}</p>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        </div>
      </div>
    </div>
  {/if}
</section>
