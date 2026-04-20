<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  type MediaType = 'podcast' | 'essay' | 'book' | 'article' | 'paper';

  interface Engager {
    pubkey: string;
    colorIndex: number;
  }

  interface Reaction {
    pubkey: string;
    memberColorIndex: number;
    text: string;
  }

  let {
    href,
    type,
    sharedBy,
    when,
    artworkLabel,
    artworkVariant = 'default',
    title,
    source,
    excerptStamp,
    excerptQuote,
    reactions = [],
    engaged = [],
    openLabel
  }: {
    href: string;
    type: MediaType;
    sharedBy: string;
    when: string;
    artworkLabel?: string;
    artworkVariant?: 'default' | 'essay';
    title: string;
    source?: string;
    excerptStamp?: string;
    excerptQuote?: string;
    reactions?: Reaction[];
    engaged?: Engager[];
    openLabel?: string;
  } = $props();

  const defaultOpenLabel = $derived(
    openLabel ?? (type === 'podcast' ? 'Open episode →' : 'Open essay →')
  );
</script>

<a {href} class="also-card">
  <div class="also-type">
    <b>{type.charAt(0).toUpperCase() + type.slice(1)}</b> · shared by {sharedBy} · {when}
  </div>

  <div class="also-head">
    <div class="also-artwork" class:essay={artworkVariant === 'essay'}>
      {#if artworkLabel}{artworkLabel}{/if}
    </div>
    <div>
      <div class="also-title">{title}</div>
      {#if source}<div class="also-source">{source}</div>{/if}
    </div>
  </div>

  {#if excerptQuote}
    <div class="also-highlight">
      {#if excerptStamp}
        <div class="also-stamp">{@html excerptStamp}</div>
      {/if}
      <p class="also-quote">{excerptQuote}</p>

      {#if reactions.length}
        <div class="also-reactions">
          {#each reactions as r (r.pubkey)}
            <User.Root {ndk} pubkey={r.pubkey}>
              <div class="r-line">
                <span
                  class="room-member-avatar"
                  style:--mav-size="22px"
                  style:--mav-ring={memberTint(r.memberColorIndex)}
                  style:--mav-ring-width="1.5px"
                >
                  <User.Avatar />
                </span>
                <span><span class="r-name"><User.Name field="displayName" /></span>{r.text}</span>
              </div>
            </User.Root>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <div class="also-foot">
    <div class="dots">
      {#each engaged as member, i (member.pubkey)}
        <span class:overlap={i > 0}>
          <User.Root {ndk} pubkey={member.pubkey}>
            <span
              class="room-member-avatar"
              style:--mav-size="22px"
              style:--mav-ring={memberTint(member.colorIndex)}
              style:--mav-ring-width="1.5px"
            >
              <User.Avatar />
            </span>
          </User.Root>
        </span>
      {/each}
    </div>
    <span class="open-arrow">{defaultOpenLabel}</span>
  </div>
</a>

<style>
  .also-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-decoration: none;
    color: inherit;
    transition: border-color 200ms ease, transform 200ms ease;
  }

  .also-card:hover {
    border-color: var(--brand-accent);
    transform: translateY(-2px);
  }

  .also-type {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  .also-type b {
    color: var(--brand-accent);
    font-weight: 500;
  }

  .also-head {
    display: grid;
    grid-template-columns: 58px 1fr;
    gap: 14px;
    margin-bottom: 6px;
  }

  .also-artwork {
    aspect-ratio: 1/1;
    background: linear-gradient(140deg, #2A3E5E 0%, #4A6B9C 100%);
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #D8E3F2;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 10px;
    line-height: 1.1;
    text-align: center;
    padding: 6px;
    font-weight: 500;
  }

  .also-artwork.essay {
    background: linear-gradient(140deg, #3E1F14 0%, #6A3922 100%);
    color: #F0D9B6;
  }

  .also-artwork.essay::before {
    content: '§';
    font-size: 22px;
    font-style: normal;
    font-weight: 300;
  }

  .also-title {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 17px;
    line-height: 1.2;
    color: var(--ink);
    letter-spacing: -0.005em;
  }

  .also-source {
    font-family: var(--font-sans);
    font-style: italic;
    font-size: 13px;
    color: var(--ink-fade);
    margin-top: -2px;
  }

  .also-highlight {
    padding: 10px 14px;
    background: var(--surface-warm);
    border-left: 3px solid var(--marker-strong);
    border-radius: 0 3px 3px 0;
  }

  .also-stamp {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--ink-fade);
    margin-bottom: 6px;
    text-transform: uppercase;
  }

  .also-stamp :global(b) {
    color: var(--brand-accent);
    font-weight: 500;
  }

  .also-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--ink);
    margin: 0 0 10px;
  }

  .also-reactions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px dashed rgba(21, 19, 15, 0.1);
  }

  .r-line {
    display: grid;
    grid-template-columns: 24px 1fr;
    gap: 10px;
    font-size: 13px;
    line-height: 1.45;
    color: var(--ink-soft);
    align-items: start;
  }

  .r-name {
    font-weight: 600;
    color: var(--ink);
    margin-right: 6px;
  }

  .also-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 10px;
    border-top: 1px dotted var(--rule);
    margin-top: auto;
  }

  .also-foot .dots {
    display: flex;
  }

  .also-foot .overlap {
    margin-left: -8px;
  }

  .also-foot :global(.room-member-avatar) {
    box-shadow: 0 0 0 1px var(--surface);
  }

  .open-arrow {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.02em;
    color: var(--brand-accent);
  }
</style>
