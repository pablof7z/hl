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
  import { buildCommunitySummary } from '$lib/ndk/groups';

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
  const communityFeed = ndk.$subscribe(() => {
    if (!currentUser) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupMetadata, NDKKind.GroupAdmins, NDKKind.GroupMembers], limit: 192 }],
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

  function latestByGroupId(kind: number): Map<string, NDKEvent> {
    const latest = new Map<string, NDKEvent>();

    for (const event of communityFeed.events.filter((candidate) => candidate.kind === kind)) {
      const groupId = event.tagValue('d')?.trim();
      if (!groupId) continue;

      const existing = latest.get(groupId);
      if (!existing || (event.created_at ?? 0) > (existing.created_at ?? 0)) {
        latest.set(groupId, event);
      }
    }

    return latest;
  }

  function includesPubkey(event: NDKEvent | undefined, pubkey: string): boolean {
    if (!event || !pubkey) return false;
    return event.getMatchingTags('p').some((tag) => tag[1] === pubkey);
  }

  const communities = $derived.by(() => {
    if (!currentUser) return [];

    const metadataByGroupId = latestByGroupId(NDKKind.GroupMetadata);
    const adminByGroupId = latestByGroupId(NDKKind.GroupAdmins);
    const memberByGroupId = latestByGroupId(NDKKind.GroupMembers);
    const joined = [];

    for (const [groupId, metadataEvent] of metadataByGroupId) {
      const adminEvent = adminByGroupId.get(groupId);
      const memberEvent = memberByGroupId.get(groupId);
      const isAdmin = includesPubkey(adminEvent, currentUser.pubkey);
      const isMember = includesPubkey(memberEvent, currentUser.pubkey);

      if (!isAdmin && !isMember) continue;

      try {
        joined.push(
          buildCommunitySummary(metadataEvent, {
            adminEvent,
            memberEvent
          })
        );
      } catch {
        continue;
      }
    }

    return joined.toSorted((left, right) => left.name.localeCompare(right.name));
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
    <p class="panel-message">No memberships loaded yet. Join or create a community first.</p>
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
