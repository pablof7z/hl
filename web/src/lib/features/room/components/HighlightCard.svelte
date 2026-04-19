<script lang="ts">
  import MemberDot from './MemberDot.svelte';

  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  interface ArtifactRef {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
    cover?: string;
  }

  let {
    id,
    quote,
    memberColorIndex,
    memberName,
    artifactTitle,
    artifact,
    onHighlightClick
  }: {
    id: string;
    quote: string;
    memberColorIndex: number;
    memberName: string;
    artifactTitle: string;
    artifact?: ArtifactRef;
    onHighlightClick?: (artifact: ArtifactRef) => void;
  } = $props();

  function handleClick() {
    if (onHighlightClick && artifact) {
      onHighlightClick(artifact);
    } else {
      console.log('highlight clicked:', id);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article
  class="highlight-card"
  role="listitem"
  data-id={id}
  onclick={handleClick}
  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(); }}
  style:cursor={onHighlightClick && artifact ? 'pointer' : 'default'}
>
  <p class="highlight-quote">{quote}</p>
  <div class="highlight-footer">
    <div class="highlight-member" aria-hidden="true">
      <MemberDot colorIndex={memberColorIndex} size="sm" />
    </div>
    <span class="highlight-name">{memberName}</span>
    <span class="highlight-artifact">{artifactTitle}</span>
  </div>
</article>

<style>
  .highlight-card {
    flex-shrink: 0;
    width: 280px;
    background-color: var(--surface);
    border: 1px solid var(--rule);
    border-left: 3px solid var(--marker-strong);
    border-radius: var(--radius, 4px);
    padding: 16px 18px 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    scroll-snap-align: start;
  }

  .highlight-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 15px;
    color: var(--ink-soft);
    line-height: 1.55;
    margin: 0;
    /* max 3 lines */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    flex: 1;
  }

  .highlight-footer {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .highlight-member {
    flex-shrink: 0;
  }

  .highlight-name {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink);
  }

  .highlight-artifact {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 400;
    color: var(--ink-fade);
    margin-left: auto;
    text-align: right;
    /* truncate long titles */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
  }
</style>
