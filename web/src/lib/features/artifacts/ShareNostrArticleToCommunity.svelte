<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { NDKKind, type NDKEvent } from '@nostr-dev-kit/ndk';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import {
    artifactPath,
    buildNostrArticleArtifactPreview,
    publishArtifact
  } from '$lib/ndk/artifacts';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { buildJoinedCommunities, groupIdFromEvent } from '$lib/ndk/groups';

  let {
    event,
    authorName = ''
  }: {
    event: NDKEvent;
    authorName?: string;
  } = $props();

  let selectedGroupId = $state('');
  let note = $state('');
  let publishing = $state(false);
  let errorMessage = $state('');
  let statusMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const membershipFeed = ndk.$subscribe(() => {
    if (!currentUser) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();

    for (const event of membershipFeed.events) {
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }

    return [...ids];
  });
  const metadataFeed = ndk.$subscribe(() => {
    if (!currentUser || membershipGroupIds.length === 0) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupMetadata], '#d': membershipGroupIds, limit: Math.max(membershipGroupIds.length * 2, 32) }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const preview = $derived(
    buildNostrArticleArtifactPreview({
      event: event.rawEvent(),
      canonicalUrl: page.url.href,
      authorName
    })
  );

  const communities = $derived.by(() => {
    if (!currentUser) return [];

    return buildJoinedCommunities(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events]);
  });
  const canShare = $derived(Boolean(currentUser && selectedGroupId && !publishing && !isReadOnly));

  $effect(() => {
    if (!selectedGroupId && communities.length > 0) {
      selectedGroupId = communities[0].id;
    }
  });

  async function handleShare() {
    if (!currentUser) {
      errorMessage = 'Sign in before sharing articles into a community.';
      return;
    }

    if (isReadOnly) {
      errorMessage = 'Read-only sessions cannot publish community share threads.';
      return;
    }

    if (!selectedGroupId) {
      errorMessage = 'Pick a community first.';
      return;
    }

    publishing = true;
    errorMessage = '';
    statusMessage = '';

    try {
      await ensureClientNdk();

      const result = await publishArtifact(ndk, {
        groupId: selectedGroupId,
        preview,
        note
      });

      statusMessage = result.existing
        ? 'That article is already shared in this community. Opening it now.'
        : 'Article shared to the community.';
      await goto(artifactPath(selectedGroupId, result.artifact.id), { invalidateAll: true });
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Could not share the article.';
    } finally {
      publishing = false;
    }
  }
</script>

<section class="share-article-panel">
  <div class="panel-copy">
    <p class="eyebrow">Share To Community</p>
    <h2>Bring this article into one of your groups.</h2>
    <p>
      This publishes a `kind:11` thread pointing at the article’s `a` coordinate, so highlights can
      be queried directly from the source.
    </p>
  </div>

  {#if !currentUser}
    <p class="panel-message">Sign in to share this article into one of your communities.</p>
  {:else if communities.length === 0}
    <div class="panel-empty">
      <p class="panel-message">No memberships loaded yet. Join or create a community first.</p>
      <div class="panel-empty-actions">
        <a href="/discover">Browse public communities</a>
        <a href="/community/create">Create a community</a>
      </div>
    </div>
  {:else}
    <div class="panel-fields">
      <label class="field">
        <span>Community</span>
        <select bind:value={selectedGroupId}>
          {#each communities as community (community.id)}
            <option value={community.id}>{community.name}</option>
          {/each}
        </select>
      </label>

      <label class="field">
        <span>Why share it?</span>
        <textarea
          bind:value={note}
          rows="3"
          maxlength="280"
          placeholder="Optional framing for this community."
        ></textarea>
      </label>

      <div class="preview-strip">
        <span>{preview.source}</span>
        <span>{preview.domain}</span>
        <span>{preview.title}</span>
      </div>

      <button type="button" disabled={!canShare} onclick={handleShare}>
        {publishing ? 'Sharing…' : 'Share Into Community'}
      </button>

      {#if errorMessage}
        <p class="error">{errorMessage}</p>
      {/if}

      {#if statusMessage}
        <p class="status">{statusMessage}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .share-article-panel {
    display: grid;
    gap: 1rem;
    padding: 1.15rem;
    border: 1px solid var(--border);
    border-radius: 1.2rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.76rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .panel-copy h2 {
    margin: 0.3rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.45rem;
    line-height: 1.12;
    letter-spacing: -0.02em;
  }

  .panel-copy p:last-child,
  .panel-message,
  .error,
  .status {
    margin: 0.55rem 0 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .panel-empty {
    display: grid;
    gap: 0.8rem;
  }

  .panel-empty-actions {
    display: flex;
    gap: 0.65rem;
    flex-wrap: wrap;
  }

  .panel-empty-actions a {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.5rem;
    padding: 0 0.9rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text);
    font-size: 0.88rem;
    font-weight: 600;
  }

  .panel-fields {
    display: grid;
    gap: 0.9rem;
  }

  .field {
    display: grid;
    gap: 0.4rem;
  }

  .field span {
    color: var(--text-strong);
    font-size: 0.88rem;
    font-weight: 700;
  }

  .field select,
  .field textarea {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    color: var(--text);
    padding: 0.85rem 0.95rem;
  }

  .field textarea {
    resize: vertical;
  }

  .preview-strip {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .preview-strip span {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
  }

  button {
    width: fit-content;
    min-height: 2.7rem;
    padding: 0 1rem;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 700;
  }

  button:disabled {
    opacity: 0.55;
  }

  .error {
    color: #b42318;
  }

  .status {
    color: #0f766e;
  }
</style>
