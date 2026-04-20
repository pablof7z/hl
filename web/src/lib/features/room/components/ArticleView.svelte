<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import MemberStack from './MemberStack.svelte';
  import FilterRow from './FilterRow.svelte';
  import AnnotationCard from './AnnotationCard.svelte';
  import DiscussionRow from './DiscussionRow.svelte';
  import { memberTint } from '../utils/colors';

  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  interface ArtifactCardProps {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
    highlightCount?: number;
    discussionCount?: number;
  }

  interface Member {
    pubkey: string;
    colorIndex: number;
    name: string;
    joinedAt?: string;
  }

  let {
    artifact,
    onBack,
    members
  }: {
    artifact: ArtifactCardProps;
    onBack: () => void;
    members: Member[];
  } = $props();

  const allMemberNames = $derived(['All', ...members.map((m) => m.name)]);
  let activePill = $state('All');

  const TINT_BG_VARS = [
    'var(--h-amber-bg)',
    'var(--h-sage-bg)',
    'var(--h-blue-bg)',
    'var(--h-rose-bg)',
    'var(--h-lilac-bg)',
    'var(--h-amber-l-bg)'
  ] as const;

  function getMemberBgColor(colorIndex: number): string {
    return TINT_BG_VARS[((colorIndex - 1) % 6 + 6) % 6];
  }

  type BodyBlock =
    | { type: 'paragraph'; text: string; memberIdx?: number }
    | { type: 'annotation'; memberIdx: number; highlight: string };

  const seedArticleBody: BodyBlock[] = [
    {
      type: 'paragraph',
      text: 'The transition from the Industrial Age to the Information Age will be as disruptive as the transition from the Agricultural to the Industrial Age. Those who understand the dynamics of this transformation will thrive; those who resist it will be left behind.',
      memberIdx: 0
    },
    {
      type: 'annotation',
      memberIdx: 0,
      highlight: '"The death of distance" — communication technology eliminates geographic constraints on economic activity.'
    },
    {
      type: 'paragraph',
      text: 'The sovereign individual will be someone who can earn a living anywhere on earth, unbound by national borders or currency controls. This is not a prediction about the future — it is a description of what is already happening in the most dynamic sectors of the global economy.',
      memberIdx: 1
    },
    {
      type: 'paragraph',
      text: 'The nation-state, as we have known it, emerged from the technological conditions of the Industrial Age. Its institutions — central banks, military establishments, education systems — were designed for a world of physical capital and geographic boundaries. In the Information Age, these institutions will become increasingly obsolete.'
    },
    {
      type: 'annotation',
      memberIdx: 1,
      highlight: 'Their framework for understanding the transition applies directly to the current era. The signs are everywhere.'
    },
    {
      type: 'paragraph',
      text: 'The most important skill for the sovereign individual is not technical knowledge but the capacity to think independently and act decisively in a world of rapid change. The institutions of the Industrial Age reward conformity; the Information Age rewards creativity.'
    }
  ];

  const seedRelatedDiscussions = $derived([
    {
      id: 'rd1',
      status: 'active' as const,
      title: 'The chapter on digital governance is the most prescient.',
      starterIdx: 0,
      replies: 12,
      lastAt: '2h ago'
    },
    {
      id: 'rd2',
      status: 'active' as const,
      title: 'Did anyone else notice the parallel with Taleb?',
      starterIdx: 1,
      replies: 8,
      lastAt: '5h ago'
    },
    {
      id: 'rd3',
      status: 'closed' as const,
      title: 'The economic predictions held up surprisingly well.',
      starterIdx: 2,
      replies: 6,
      lastAt: '1d ago'
    }
  ]);

  function memberAt(idx: number | undefined): Member | undefined {
    if (idx === undefined) return undefined;
    return members[idx];
  }

  function handleSaveForLater() {
    console.log('save for later:', artifact.id);
  }

  function handleShare() {
    console.log('share:', artifact.id);
  }
</script>

<article class="article-view">
  <!-- Back button -->
  <div class="article-nav">
    <button class="back-btn" type="button" onclick={onBack}>
      ← Back to room
    </button>
  </div>

  <!-- Hero -->
  <header class="article-hero">
    {#if artifact.cover}
      <img
        class="hero-cover"
        src={artifact.cover}
        alt=""
        loading="eager"
      />
    {/if}

    <div class="hero-meta">
      <span class="article-kicker">ARTICLE</span>
      <h1 class="article-title">{artifact.title}</h1>
      {#if artifact.author}
        <p class="article-author">{artifact.author}</p>
      {/if}
      <div class="members-strip">
        <MemberStack {members} />
        <span class="members-label">{members.length} members reading</span>
      </div>
    </div>
  </header>

  <!-- Body + margin column -->
  <div class="article-body-layout">
    <!-- Main article body -->
    <div class="article-body">
      {#each seedArticleBody as block, i (i)}
        {#if block.type === 'paragraph'}
          {@const marker = memberAt(block.memberIdx)}
          {#if marker}
            {@const color = memberTint(marker.colorIndex)}
            {@const bg = getMemberBgColor(marker.colorIndex)}
            <p class="body-paragraph">
              <mark
                class="inline-mark"
                style:background={bg}
                style:border-left="3px solid {color}"
                title="{marker.name} highlighted this"
              >{block.text}</mark>
              <span class="mark-tooltip" aria-hidden="true">{marker.name}</span>
            </p>
          {:else}
            <p class="body-paragraph">{block.text}</p>
          {/if}
        {:else if block.type === 'annotation'}
          {@const annotator = memberAt(block.memberIdx)}
          {#if annotator}
            <AnnotationCard
              pubkey={annotator.pubkey}
              colorIndex={annotator.colorIndex}
              highlight={block.highlight}
            />
          {/if}
        {/if}
      {/each}

      <!-- Article footer -->
      <div class="article-footer">
        <button class="save-btn" type="button" onclick={handleSaveForLater}>
          Save for later
        </button>
        <button class="share-link" type="button" onclick={handleShare}>
          Share →
        </button>
      </div>
    </div>

    <!-- Margin column -->
    <aside class="article-margin">
      <div class="margin-section">
        <FilterRow
          pills={allMemberNames}
          activePill={activePill}
          onToggle={(label) => (activePill = label)}
        />
      </div>

      <div class="margin-card">
        <h3 class="margin-card-title">Members in this article</h3>
        <div class="members-grid">
          {#each members as member (member.pubkey)}
            <User.Root {ndk} pubkey={member.pubkey}>
              <div class="member-item">
                <span
                  class="room-member-avatar"
                  style:--mav-size="24px"
                  style:--mav-ring={memberTint(member.colorIndex)}
                  style:--mav-ring-width="1.5px"
                >
                  <User.Avatar />
                </span>
                <span class="member-item-name"><User.Name field="displayName" /></span>
              </div>
            </User.Root>
          {/each}
        </div>
      </div>

      <div class="margin-card">
        <h3 class="margin-card-title">Related discussions</h3>
        <div class="related-list">
          {#each seedRelatedDiscussions as disc (disc.id)}
            {@const starter = memberAt(disc.starterIdx)}
            {#if starter}
              <DiscussionRow
                id={disc.id}
                status={disc.status}
                title={disc.title}
                starterPubkey={starter.pubkey}
                participants={[{ pubkey: starter.pubkey, colorIndex: starter.colorIndex }]}
                replies={disc.replies}
                lastAt={disc.lastAt}
              />
            {/if}
          {/each}
        </div>
      </div>

      <div class="margin-card margin-footer-card">
        <button class="save-btn" type="button" onclick={handleSaveForLater}>
          Save for later
        </button>
        <button class="share-link" type="button" onclick={handleShare}>
          Share →
        </button>
      </div>
    </aside>
  </div>
</article>

<style>
  .article-view {
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding-top: 24px;
    padding-bottom: 80px;
  }

  .article-nav {
    padding: 0;
  }

  .back-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink-soft);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    transition: color var(--transition);
  }

  .back-btn:hover {
    color: var(--brand-accent);
  }

  .back-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }

  /* Hero */
  .article-hero {
    display: flex;
    flex-direction: column;
    gap: 40px;
    align-items: flex-start;
  }

  .hero-cover {
    width: 100%;
    max-width: 100%;
    border-radius: var(--radius);
    object-fit: cover;
    aspect-ratio: 16/9;
    flex-shrink: 0;
  }

  .hero-meta {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding-top: 8px;
  }

  .article-kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .article-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(32px, 5vw, 56px);
    color: var(--ink);
    line-height: 1.15;
    margin: 0;
  }

  .article-author {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 400;
    color: var(--ink-soft);
    margin: 0;
  }

  .members-strip {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  .members-label {
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ink-fade);
  }

  /* Body layout */
  .article-body-layout {
    display: grid;
    grid-template-columns: 1fr;
    gap: 40px;
    align-items: start;
  }

  .article-body {
    max-width: 100%;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .body-paragraph {
    font-family: var(--font-serif);
    font-size: 18px;
    line-height: 1.65;
    color: var(--ink);
    margin: 0;
    position: relative;
  }

  .inline-mark {
    padding: 0 4px;
    background: transparent; /* overridden inline */
    border-left: 3px solid transparent; /* overridden inline */
    padding-left: 8px;
    cursor: default;
  }

  .mark-tooltip {
    display: none;
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    background-color: var(--surface);
    border: 1px solid var(--brand-accent);
    border-radius: var(--radius);
    padding: 4px 10px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 13px;
    color: var(--ink);
    white-space: nowrap;
    pointer-events: none;
    z-index: 5;
  }

  .body-paragraph:hover .mark-tooltip {
    display: block;
  }

  /* Article footer */
  .article-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 24px;
    border-top: 1px solid var(--rule);
    margin-top: 8px;
  }

  .save-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--surface);
    background-color: var(--brand-accent);
    border: none;
    border-radius: var(--radius);
    padding: 10px 20px;
    cursor: pointer;
    transition: opacity var(--transition);
  }

  .save-btn:hover {
    opacity: 0.85;
  }

  .share-link {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .share-link:hover {
    text-decoration: underline;
  }

  /* Margin column */
  .article-margin {
    position: static;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .margin-section {
    margin-bottom: 4px;
  }

  .margin-card {
    background-color: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .margin-card-title {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0;
  }

  .members-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .member-item {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .member-item-name {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .related-list {
    display: flex;
    flex-direction: column;
  }

  .margin-footer-card {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  @media (min-width: 768px) {
    .article-hero {
      flex-direction: row;
    }

    .hero-cover {
      width: 60%;
      max-width: 520px;
    }

    .article-body {
      max-width: 680px;
      margin: 0 auto;
    }

    .article-body-layout {
      grid-template-columns: 1fr 220px;
    }

    .article-margin {
      position: sticky;
      top: 24px;
    }
  }
</style>
