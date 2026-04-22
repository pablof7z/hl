<script lang="ts">
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { createFetchUser } from '@nostr-dev-kit/svelte';
  import type { NDKEvent, NDKUserProfile } from '@nostr-dev-kit/ndk';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import type { DiscussionRootContext } from '$lib/features/discussions/discussion';
  import ArticleMarkdown from '$lib/components/ArticleMarkdown.svelte';
  import * as Tabs from '$lib/components/ui/tabs';
  import { User } from '$lib/ndk/ui/user';
  import {
    articlePublishedAt,
    articleReadTimeMinutes,
    articleSummary,
    articleTitle,
    displayNip05,
    displayName,
    formatDisplayDate,
    noteExcerpt,
    noteTitle,
    profileIdentifier
  } from '$lib/ndk/format';
  import { safeUserIdentifier } from '$lib/ndk/user';
  import { ndk } from '$lib/ndk/client';
  import {
    BOOKMARK_LIST_KIND,
    bookmarkListHasAddress,
    latestListEvent,
    setBookmarkAddressPresence
  } from '$lib/ndk/lists';
  import { MarkdownEventContent } from '$lib/ndk/ui/markdown-event-content';
  import '$lib/ndk/components/mention';
  import '$lib/ndk/components/embedded-note';
  import '$lib/ndk/components/embedded-article';
  import EventAuthorHeader from '$lib/components/EventAuthorHeader.svelte';
  import BookmarkIcon from '$lib/components/BookmarkIcon.svelte';
  import HighlightPopover from '$lib/components/HighlightPopover.svelte';
  import SharePopover from '$lib/components/SharePopover.svelte';
  import ShareNostrArticleToRoom from '$lib/features/artifacts/ShareNostrArticleToRoom.svelte';
  import DiscussionPanel from '$lib/features/discussions/DiscussionPanel.svelte';
  import RoomContextBar from './RoomContextBar.svelte';
  import { mergeUniqueEvents } from '$lib/ndk/events';
  import {
    type CommentNode,
    buildArticleCommentTree,
    targetReferences,
    buildReferenceFilters
  } from './comments';

  interface CommunityContext {
    groupId: string;
    roomName: string;
    roomUrl: string;
    artifact?: ArtifactRecord;
    rootContext: DiscussionRootContext;
  }

  let {
    event,
    authorPubkey: authorPubkeyProp = undefined,
    authorProfile: authorProfileProp = undefined,
    authorLinkIdentifier: authorLinkIdentifierProp = undefined,
    seedComments = [],
    seedHighlights = [],
    highlightEvents: highlightEventsProp = undefined,
    roomContext = undefined
  }: {
    event: NDKEvent;
    authorPubkey?: string;
    authorProfile?: NDKUserProfile;
    authorLinkIdentifier?: string;
    seedComments?: NDKEvent[];
    seedHighlights?: NDKEvent[];
    highlightEvents?: NDKEvent[];
    roomContext?: CommunityContext;
  } = $props();

  const authorPubkey = $derived(authorPubkeyProp ?? event.pubkey ?? '');
  const internalAuthor = createFetchUser(ndk, () => (authorProfileProp ? '' : authorPubkey));
  const authorProfile = $derived(authorProfileProp ?? internalAuthor.profile);
  const authorLinkIdentifier = $derived(
    authorLinkIdentifierProp ?? profileIdentifier(authorProfile, safeUserIdentifier(internalAuthor, authorPubkey))
  );

  // In standalone mode, subscribe to comments and highlights from the network.
  // In room mode, highlights come in via highlightEventsProp; DiscussionPanel handles comments.
  const liveComments = ndk.$subscribe(() => {
    if (!browser || roomContext || event.kind !== 30023) return undefined;
    const filters = buildReferenceFilters(targetReferences(event), [1111], {
      addressTag: 'A', idTag: 'E', limit: 120
    });
    return filters.length > 0 ? { filters } : undefined;
  });

  const liveHighlights = ndk.$subscribe(() => {
    if (!browser || roomContext || highlightEventsProp || event.kind !== 30023) return undefined;
    const filters = buildReferenceFilters(targetReferences(event), [9802], {
      addressTag: 'a', idTag: 'e', limit: 80
    });
    return filters.length > 0 ? { filters } : undefined;
  });

  const commentEvents = $derived(
    roomContext
      ? undefined
      : mergeUniqueEvents(liveComments.events.filter((e) => e.kind === 1111), seedComments)
  );

  const highlightEvents = $derived(
    highlightEventsProp ??
    mergeUniqueEvents(liveHighlights.events.filter((e) => e.kind === 9802), seedHighlights)
  );

  type Lens = 'room' | 'rooms' | 'network';
  let activeTab = $state<'article' | 'comments' | 'highlights'>('article');
  let activeLens = $state<Lens>('room');
  let replyingTo = $state<string | null>(null);
  let replyText = $state('');
  let submitting = $state(false);
  let bookmarking = $state(false);
  let articleContentEl = $state<HTMLElement | null>(null);

  const currentUser = $derived(ndk.$currentUser);
  const isArticle = $derived(event.kind === 30023);

  // Bookmark logic
  const bookmarkList = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [{ kinds: [BOOKMARK_LIST_KIND], authors: [currentUser.pubkey], limit: 20 }]
    };
  });
  const bookmarkListEvent = $derived(latestListEvent(bookmarkList.events));

  const isBookmarked = $derived.by(() => {
    const addr = event.tagId();
    return bookmarkListHasAddress(bookmarkListEvent, addr);
  });

  async function toggleBookmark() {
    if (!currentUser || bookmarking) return;
    bookmarking = true;
    try {
      const addr = event.tagId();
      await setBookmarkAddressPresence(ndk, bookmarkListEvent, addr, !isBookmarked);
    } finally {
      bookmarking = false;
    }
  }

  // Derived display values
  const authorName = $derived(
    displayName(authorProfile, authorPubkey ? `${authorPubkey.slice(0, 8)}...` : 'Author')
  );
  const authorIdentity = $derived.by(() => {
    const nip05 = displayNip05(authorProfile);
    return nip05 && nip05 !== authorName ? nip05 : '';
  });
  const articleAddress = $derived(isArticle ? event.tagId() : '');
  const shareUrl = $derived(browser ? page.url.href : '');
  const articleEventId = $derived(event.id ?? '');
  const commentCount = $derived(commentEvents?.length ?? 0);
  const highlightCount = $derived(highlightEvents.length);

  // Comment tree (standalone mode only)
  const commentTree = $derived.by(() => {
    if (!commentEvents) return [];
    return buildArticleCommentTree(commentEvents, articleAddress, articleEventId);
  });

  // Comment submission (standalone mode only)
  async function submitComment(parentEvent: NDKEvent | null) {
    if (!currentUser || !replyText.trim()) return;

    submitting = true;
    try {
      const root = event;
      let reply: NDKEvent;

      if (parentEvent) {
        reply = parentEvent.reply();
      } else {
        reply = root.reply();
      }

      reply.content = replyText.trim();
      await reply.publish();

      replyText = '';
      replyingTo = null;
    } finally {
      submitting = false;
    }
  }

  // Check for ?tab=discussion on mount (room mode)
  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('tab') === 'discussion' || params.get('tab') === 'comments') {
      activeTab = 'comments';
      history.replaceState(null, '', window.location.pathname);
    }
  });
</script>

<section class="mx-auto grid max-w-[var(--content-width)] gap-5">
  {#if roomContext}
    <RoomContextBar
      roomName={roomContext.roomName}
      roomUrl={roomContext.roomUrl}
      {activeLens}
      onLensChange={(lens) => (activeLens = lens)}
    />
  {/if}

  <article>
    <h1>{isArticle ? articleTitle(event.rawEvent()) : noteTitle(event.rawEvent())}</h1>

    <div class="article-byline">
      <User.Root {ndk} pubkey={authorPubkey} profile={authorProfile}>
        <a class="article-author-link" href={`/profile/${authorLinkIdentifier}`}>
          <User.Avatar class="article-author-avatar" />
        </a>
        <div class="article-author-copy">
          <div class="feed-meta">
            <a class="article-author-name" href={`/profile/${authorLinkIdentifier}`}>{authorName}</a>
            <span>
              {#if isArticle}
                {formatDisplayDate(articlePublishedAt(event.rawEvent()))}
              {:else if event.created_at}
                {new Date(event.created_at * 1000).toLocaleString()}
              {/if}
            </span>
            {#if isArticle}
              <span>{articleReadTimeMinutes(event.content)} min read</span>
            {/if}
          </div>
          {#if authorIdentity}
            <div class="feed-meta">
              <span class="article-author-handle">{authorIdentity}</span>
            </div>
          {/if}
        </div>
      </User.Root>

      <div class="ml-auto flex shrink-0 items-center gap-2">
        {#if isArticle}
          <SharePopover
            url={shareUrl}
            title={isArticle ? articleTitle(event.rawEvent()) : noteTitle(event.rawEvent())}
          />
          <ShareNostrArticleToRoom {event} authorName={authorName} />
        {/if}
        {#if currentUser && isArticle}
          <button
            class="btn btn-circle btn-outline border-base-300 text-base-content/60 transition-all hover:border-primary hover:text-primary active:scale-90 disabled:opacity-50 {isBookmarked ? '!border-primary !bg-primary/5 !text-primary' : ''}"
            disabled={bookmarking}
            title={isBookmarked ? 'Remove bookmark' : 'Bookmark this article'}
            onclick={toggleBookmark}
          >
            <BookmarkIcon filled={isBookmarked} />
          </button>
        {/if}
      </div>
    </div>

    <p class="lede" style="margin: 0;">
      {isArticle ? articleSummary(event.rawEvent(), 320) : noteExcerpt(event.content, 320)}
    </p>

    {#if isArticle}
      <Tabs.Root bind:value={activeTab} activationMode="manual">
        <Tabs.List class="article-tabs-list" aria-label="Article views">
          <Tabs.Trigger value="article">Article</Tabs.Trigger>
          <Tabs.Trigger value="comments">
            <span>Comments</span>
            {#if !roomContext}
              <span class="article-tab-count">{commentCount}</span>
            {/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="highlights">
            <span>Highlights</span>
            <span class="article-tab-count">{highlightCount}</span>
          </Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="article" class="article-tab-panel">
          <div class="card card-border bg-base-100 p-6" bind:this={articleContentEl}>
            <ArticleMarkdown content={event.content} tags={event.tags} highlights={highlightEvents} />
          </div>
          <div class="mt-10 flex flex-col items-center gap-4 border-t border-base-300 pt-10 pb-4">
            <p class="m-0 text-xs font-semibold uppercase tracking-wider text-base-content/60">
              Share this article
            </p>
            <div class="flex flex-wrap justify-center gap-2.5">
              <a
                class="btn btn-sm btn-outline gap-2 rounded-full"
                href="https://x.com/intent/tweet?text={encodeURIComponent(articleTitle(event.rawEvent()))}&url={encodeURIComponent(shareUrl)}"
                target="_blank"
                rel="noopener noreferrer"
              >
                <svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-4.714-6.231-5.401 6.231H2.744l7.737-8.835L1.254 2.25H8.08l4.253 5.622zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
                </svg>
                X / Twitter
              </a>
              <a
                class="btn btn-sm btn-outline gap-2 rounded-full"
                href="https://www.facebook.com/sharer/sharer.php?u={encodeURIComponent(shareUrl)}"
                target="_blank"
                rel="noopener noreferrer"
              >
                <svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M24 12.073c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.99 4.388 10.954 10.125 11.854v-8.385H7.078v-3.47h3.047V9.43c0-3.007 1.792-4.669 4.533-4.669 1.312 0 2.686.235 2.686.235v2.953H15.83c-1.491 0-1.956.925-1.956 1.874v2.25h3.328l-.532 3.47h-2.796v8.385C19.612 23.027 24 18.062 24 12.073z"/>
                </svg>
                Facebook
              </a>
              <a
                class="btn btn-sm btn-outline gap-2 rounded-full"
                href="https://www.linkedin.com/sharing/share-offsite/?url={encodeURIComponent(shareUrl)}"
                target="_blank"
                rel="noopener noreferrer"
              >
                <svg class="size-4" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433a2.062 2.062 0 0 1-2.063-2.065 2.064 2.064 0 1 1 2.063 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/>
                </svg>
                LinkedIn
              </a>
            </div>
          </div>
        </Tabs.Content>

        <Tabs.Content value="comments" class="article-tab-panel">
          {#if roomContext}
            <DiscussionPanel
              groupId={roomContext.groupId}
              rootContext={roomContext.rootContext}
              showHeader={false}
            />
          {:else}
            <!-- Standalone comment tree -->
            {#if currentUser}
              {#if replyingTo === null}
                <div class="mb-6 grid gap-2.5">
                  <textarea
                    class="textarea w-full"
                    placeholder="Write a comment..."
                    bind:value={replyText}
                    rows="3"
                  ></textarea>
                  <div class="flex justify-end">
                    <button
                      class="btn btn-primary"
                      disabled={submitting || !replyText.trim()}
                      onclick={() => submitComment(null)}
                    >
                      {submitting ? 'Publishing...' : 'Comment'}
                    </button>
                  </div>
                </div>
              {/if}
            {:else}
              <p class="mb-6 text-base-content/60">Log in to leave a comment.</p>
            {/if}

            {#if commentTree.length > 0}
              <div class="grid gap-0">
                {#snippet renderComments(nodes: CommentNode[], depth = 0)}
                  {#each nodes as node (node.event.id)}
                    <div
                      class="grid grid-cols-[2rem_1fr] gap-x-3 gap-y-0 py-4"
                      class:border-t={depth === 0}
                      class:border-base-300={depth === 0}
                      class:first:border-t-0={depth === 0}
                    >
                      <div class="col-span-2 mb-2 flex items-center gap-2.5">
                        <EventAuthorHeader
                          {ndk}
                          pubkey={node.event.pubkey}
                          timestamp={node.event.created_at}
                          fallbackName="Commenter"
                          avatarClass="article-author-avatar !size-8"
                        />
                      </div>

                      <div class="col-span-2 grid gap-2 pl-11">
                        <MarkdownEventContent
                          {ndk}
                          content={node.event.content}
                          emojiTags={node.event.tags}
                          class="block text-sm leading-relaxed text-base-content [overflow-wrap:anywhere] [&_p]:m-0 [&_p]:mb-2 [&_p:last-child]:mb-0"
                        />

                        <div class="flex gap-3">
                          {#if currentUser}
                            <button
                              class="cursor-pointer border-0 bg-transparent p-0 text-xs font-semibold tracking-wide text-base-content/60 transition-colors hover:text-primary"
                              onclick={() => {
                                replyingTo = replyingTo === node.event.id ? null : node.event.id;
                                replyText = '';
                              }}
                            >
                              {replyingTo === node.event.id ? 'Cancel' : 'Reply'}
                            </button>
                          {/if}
                        </div>

                        {#if replyingTo === node.event.id}
                          <div class="mt-3 grid gap-2.5">
                            <textarea
                              class="textarea w-full"
                              placeholder="Write a reply..."
                              bind:value={replyText}
                              rows="3"
                            ></textarea>
                            <div class="flex justify-end">
                              <button
                                class="btn btn-primary"
                                disabled={submitting || !replyText.trim()}
                                onclick={() => submitComment(node.event)}
                              >
                                {submitting ? 'Publishing...' : 'Reply'}
                              </button>
                            </div>
                          </div>
                        {/if}

                        {#if node.children.length > 0}
                          <div class="mt-1 border-l-2 border-base-300 pl-4 [&>*+*]:border-t [&>*+*]:border-base-300">
                            {@render renderComments(node.children, depth + 1)}
                          </div>
                        {/if}
                      </div>
                    </div>
                  {/each}
                {/snippet}

                {@render renderComments(commentTree)}
              </div>
            {:else if currentUser === undefined}
              <!-- still loading -->
            {:else}
              <p class="m-0 text-base-content/60">No comments yet. Be the first.</p>
            {/if}
          {/if}
        </Tabs.Content>

        <Tabs.Content value="highlights" class="article-tab-panel">
          {#if highlightEvents.length > 0}
            <div class="highlight-stack">
              {#each highlightEvents as highlight (highlight.id)}
                <article class="card card-border bg-base-100 p-4 gap-3">
                  <EventAuthorHeader
                    {ndk}
                    pubkey={highlight.pubkey}
                    timestamp={highlight.created_at}
                    fallbackName="Reader"
                  />

                  <blockquote class="highlight-quote">
                    {highlight.content || 'This highlight has no text excerpt.'}
                  </blockquote>

                  {#if highlight.tagValue('comment')}
                    <p class="highlight-note">{highlight.tagValue('comment')}</p>
                  {/if}

                  {#if highlight.tagValue('context')}
                    <p class="caption" style="margin: 0;">Context: {highlight.tagValue('context')}</p>
                  {/if}
                </article>
              {/each}
            </div>
          {/if}
        </Tabs.Content>
      </Tabs.Root>
    {:else}
      <pre class="document-copy">{event.content}</pre>
    {/if}
  </article>

  {#if isArticle}
    <HighlightPopover
      articleEvent={event}
      containerEl={articleContentEl}
      groupId={roomContext?.groupId ?? ''}
      artifact={roomContext?.artifact}
    />
  {/if}
</section>

