<script lang="ts">
  import { ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
  import { memberTint } from '../utils/colors';

  interface Mark {
    pubkey: string;
    colorIndex: number;
  }

  let {
    id,
    quote,
    sourceTitle,
    sourceSub,
    marks = [],
    replies,
    hot = false,
    date,
    href = '#'
  }: {
    id?: string;
    quote: string;
    sourceTitle: string;
    sourceSub?: string;
    marks?: Mark[];
    replies?: number;
    hot?: boolean;
    date?: string;
    href?: string;
  } = $props();
</script>

<a {href} class="hr-card" data-id={id}>
  <p class="hr-quote">{quote}</p>
  <div class="hr-meta">
    <div class="hr-source">
      <b>{sourceTitle}</b>
      {#if sourceSub}<span class="sc">{sourceSub}</span>{/if}
    </div>
    <div class="hr-marks">
      <div class="dots">
        {#each marks as mark, i (mark.pubkey)}
          <span class:overlap={i > 0}>
            <User.Root {ndk} pubkey={mark.pubkey}>
              <span
                class="room-member-avatar"
                style:--mav-size="20px"
                style:--mav-ring={memberTint(mark.colorIndex)}
                style:--mav-ring-width="1.5px"
              >
                <User.Avatar />
              </span>
            </User.Root>
          </span>
        {/each}
      </div>
      {#if replies !== undefined}
        <div class="hr-replies">
          <b>{replies}</b> {replies === 1 ? 'reply' : 'replies'}{#if hot} · hot{/if}
        </div>
      {/if}
      {#if date}<div class="hr-date">{date}</div>{/if}
    </div>
  </div>
</a>

<style>
  .hr-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 20px 22px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    transition: border-color 200ms, transform 200ms;
    text-decoration: none;
    color: inherit;
  }

  .hr-card:hover {
    border-color: var(--brand-accent);
    transform: translateY(-2px);
  }

  .hr-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 17px;
    line-height: 1.5;
    color: var(--ink);
    margin: 0;
    padding: 0 4px;
    background: linear-gradient(180deg, transparent 60%, rgba(245, 216, 150, 0.55) 60%);
    display: inline;
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }

  .hr-quote::before { content: '\201C'; color: var(--brand-accent); margin-right: 1px; }
  .hr-quote::after  { content: '\201D'; color: var(--brand-accent); margin-left: 1px; }

  .hr-meta {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    padding-top: 12px;
    border-top: 1px dotted rgba(21, 19, 15, 0.08);
    gap: 10px;
    margin-top: auto;
  }

  .hr-source {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
    line-height: 1.4;
    flex: 1;
    min-width: 0;
  }

  .hr-source b { color: var(--ink); font-weight: 600; display: block; }
  .hr-source .sc { font-style: italic; }

  .hr-marks {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    flex-shrink: 0;
  }

  .dots { display: flex; }
  .overlap { margin-left: -5px; }
  .hr-marks :global(.room-member-avatar) { box-shadow: 0 0 0 1px var(--surface); }

  .hr-replies {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
  }

  .hr-replies b { color: var(--brand-accent); font-weight: 500; }

  .hr-date {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-fade);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
</style>
