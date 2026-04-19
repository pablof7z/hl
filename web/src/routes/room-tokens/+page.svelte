<script lang="ts">
  import '$lib/features/room/styles/tokens.css';
  import MemberDot from '$lib/features/room/components/MemberDot.svelte';
  import MemberStack from '$lib/features/room/components/MemberStack.svelte';
  import FilterPill from '$lib/features/room/components/FilterPill.svelte';
  import FilterRow from '$lib/features/room/components/FilterRow.svelte';
  import Block from '$lib/features/room/components/Block.svelte';

  const palette = [
    { name: '--bg',           hex: '#FAFAF7', label: 'Page background' },
    { name: '--surface',      hex: '#FFFFFF', label: 'Cards / panels' },
    { name: '--surface-warm', hex: '#F5EFE0', label: 'Accent surface' },
    { name: '--surface-muted',hex: '#F3F2EE', label: 'Tracks / pill bg' },
    { name: '--rule',         hex: '#E5E0D0', label: 'Default border' },
    { name: '--rule-soft',    hex: '#EFEAD9', label: 'Soft divider' },
    { name: '--ink',          hex: '#15130F', label: 'Primary text' },
    { name: '--ink-soft',     hex: '#3A362E', label: 'Body text' },
    { name: '--ink-fade',     hex: '#7A7468', label: 'Labels / metadata' },
    { name: '--brand-accent', hex: '#C24D2C', label: 'Terracotta accent' },
    { name: '--marker',       hex: '#F5D896', label: 'Honey-amber highlight' },
    { name: '--marker-strong',hex: '#E8B96A', label: 'Highlight border' },
    { name: '--h-amber',      hex: '#F5D896', label: 'Member tint 1' },
    { name: '--h-sage',       hex: '#C8D4B5', label: 'Member tint 2' },
    { name: '--h-blue',       hex: '#BCD0E0', label: 'Member tint 3' },
    { name: '--h-rose',       hex: '#EAC6C8', label: 'Member tint 4' },
    { name: '--h-lilac',      hex: '#D0C4E0', label: 'Member tint 5' },
    { name: '--h-amber-l',    hex: '#F5E6A8', label: 'Member tint 6' },
  ];

  const spacing = [
    { name: '--container-max', value: '1440px' },
    { name: '--container-px',  value: '40px (20px mobile)' },
    { name: '--grid-sidebar',  value: '380px' },
    { name: '--grid-gap',      value: '44px' },
    { name: '--block-spacing', value: '44px' },
    { name: '--scroll-margin', value: '120px' },
    { name: '--radius',        value: '4px' },
    { name: '--radius-pill',   value: '999px' },
    { name: '--transition',    value: '200ms ease-out' },
  ];

  let activePill = $state<string | undefined>('All');
  const filterPills = ['All', 'Articles', 'Podcasts', 'Highlights', 'Notes'];

  const members3  = [1, 2, 3].map(i => ({ colorIndex: i }));
  const members6  = [1, 2, 3, 4, 5, 6].map(i => ({ colorIndex: i }));
  const members9  = [1, 2, 3, 4, 5, 6, 7, 8, 9].map(i => ({ colorIndex: i }));
</script>

<svelte:head>
  <title>Room Design Tokens</title>
</svelte:head>

<div class="token-page">
  <h1>Room UI — M1 Design Tokens</h1>

  <!-- ── Palette ──────────────────────────────────────────────────────────── -->
  <Block id="palette">
    <h2>Palette</h2>
    <div class="swatch-grid">
      {#each palette as swatch (swatch.name)}
        <div class="swatch">
          <div class="swatch-color" style:background-color={swatch.hex}></div>
          <div class="swatch-meta">
            <code class="swatch-token">{swatch.name}</code>
            <span class="swatch-hex">{swatch.hex}</span>
            <span class="swatch-label">{swatch.label}</span>
          </div>
        </div>
      {/each}
    </div>
  </Block>

  <!-- ── Typography ───────────────────────────────────────────────────────── -->
  <Block id="typography">
    <h2>Typography</h2>
    <div class="type-stack">
      <div class="type-sample">
        <span class="type-label">Room title · Fraunces 400, clamp(44px,6vw,68px)</span>
        <p class="type-room-title">The Reading Room</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Section h2 · Inter 700, 19px</span>
        <p class="type-section-h2">Recent Highlights</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Artifact title · Inter 600, 17px</span>
        <p class="type-artifact-title">The Nature of Intelligence</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Label / kicker · JetBrains Mono 400, 11px, uppercase</span>
        <p class="type-label-kicker">Podcast · Episode 42</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Body content · Fraunces 400, 17–22px, lh 1.55–1.72</span>
        <p class="type-body">The best ideas tend to surface through conversation — not search. A room is a place where shared curiosity becomes collective understanding.</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Highlight quote · Fraunces italic, 16–22px</span>
        <p class="type-highlight-quote">"Attention is the beginning of devotion."</p>
      </div>

      <div class="type-sample">
        <span class="type-label">Member status · Fraunces italic, 13px</span>
        <p class="type-member-status">reading now</p>
      </div>
    </div>
  </Block>

  <!-- ── Spacing ───────────────────────────────────────────────────────────── -->
  <Block id="spacing">
    <h2>Spacing &amp; Grid</h2>
    <table class="spacing-table">
      <thead>
        <tr><th>Token</th><th>Value</th><th>Visual</th></tr>
      </thead>
      <tbody>
        {#each spacing as s (s.name)}
          <tr>
            <td><code>{s.name}</code></td>
            <td>{s.value}</td>
            <td>
              {#if s.name !== '--transition'}
                <div class="spacing-bar" style:width="min({s.value.split(' ')[0]}, 100%)"></div>
              {:else}
                <span class="ink-fade">{s.value}</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </Block>

  <!-- ── MemberDot ─────────────────────────────────────────────────────────── -->
  <Block id="member-dot">
    <h2>MemberDot</h2>
    <div class="component-row">
      <div class="component-group">
        <span class="type-label-kicker">All 6 colours (size md)</span>
        <div class="dot-row">
          {#each [1, 2, 3, 4, 5, 6] as ci (ci)}
            <MemberDot colorIndex={ci} size="md" />
          {/each}
        </div>
      </div>

      <div class="component-group">
        <span class="type-label-kicker">3 sizes (colorIndex 1)</span>
        <div class="dot-row" style="align-items: center;">
          <MemberDot colorIndex={1} size="sm" />
          <MemberDot colorIndex={1} size="md" />
          <MemberDot colorIndex={1} size="lg" />
        </div>
      </div>
    </div>
  </Block>

  <!-- ── MemberStack ───────────────────────────────────────────────────────── -->
  <Block id="member-stack">
    <h2>MemberStack</h2>
    <div class="component-group">
      <span class="type-label-kicker">3 members</span>
      <MemberStack members={members3} />
    </div>
    <div class="component-group" style="margin-top: 16px;">
      <span class="type-label-kicker">6 members</span>
      <MemberStack members={members6} />
    </div>
    <div class="component-group" style="margin-top: 16px;">
      <span class="type-label-kicker">9 members (max=6, shows +3)</span>
      <MemberStack members={members9} max={6} />
    </div>
  </Block>

  <!-- ── FilterPill ───────────────────────────────────────────────────────── -->
  <Block id="filter-pill">
    <h2>FilterPill</h2>
    <div class="component-row">
      <div class="component-group">
        <span class="type-label-kicker">Inactive</span>
        <FilterPill label="Articles" active={false} />
      </div>
      <div class="component-group">
        <span class="type-label-kicker">Active</span>
        <FilterPill label="Articles" active={true} />
      </div>
    </div>
  </Block>

  <!-- ── FilterRow ────────────────────────────────────────────────────────── -->
  <Block id="filter-row">
    <h2>FilterRow</h2>
    <div class="component-group">
      <span class="type-label-kicker">Interactive (click to toggle)</span>
      <FilterRow
        pills={filterPills}
        activePill={activePill}
        onToggle={(label) => { activePill = activePill === label ? undefined : label; }}
      />
    </div>
  </Block>
</div>

<style>
  .token-page {
    background: var(--bg);
    color: var(--ink);
    font-family: var(--font-sans);
    min-height: 100vh;
    padding: var(--container-px);
    max-width: var(--container-max);
    margin: 0 auto;
  }

  h1 {
    font-family: var(--font-serif);
    font-size: clamp(44px, 6vw, 68px);
    font-weight: 400;
    letter-spacing: -0.025em;
    line-height: 1.02;
    color: var(--ink);
    margin-bottom: 48px;
    margin-top: 0;
  }

  h2 {
    font-family: var(--font-sans);
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.018em;
    line-height: 1.15;
    color: var(--ink);
    margin-bottom: 20px;
    margin-top: 0;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule);
  }

  /* ── Swatch grid ────────────────────────────────────────────────────────── */
  .swatch-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px;
  }

  .swatch {
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--surface);
  }

  .swatch-color {
    height: 64px;
    width: 100%;
  }

  .swatch-meta {
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .swatch-token {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--brand-accent);
  }

  .swatch-hex {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-soft);
  }

  .swatch-label {
    font-size: 11px;
    color: var(--ink-fade);
  }

  /* ── Typography samples ─────────────────────────────────────────────────── */
  .type-stack {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .type-sample {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-bottom: 24px;
    border-bottom: 1px solid var(--rule-soft);
  }

  .type-sample:last-child {
    border-bottom: none;
  }

  .type-label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  .type-room-title {
    font-family: var(--font-serif);
    font-size: clamp(44px, 6vw, 68px);
    font-weight: 400;
    letter-spacing: -0.025em;
    line-height: 1.02;
    color: var(--ink);
    margin: 0;
  }

  .type-section-h2 {
    font-family: var(--font-sans);
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.018em;
    line-height: 1.15;
    color: var(--ink);
    margin: 0;
  }

  .type-artifact-title {
    font-family: var(--font-sans);
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--ink);
    margin: 0;
  }

  .type-label-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.15em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  .type-body {
    font-family: var(--font-serif);
    font-size: 17px;
    font-weight: 400;
    line-height: 1.65;
    color: var(--ink-soft);
    margin: 0;
  }

  .type-highlight-quote {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 20px;
    color: var(--ink-soft);
    margin: 0;
    padding-left: 16px;
    border-left: 3px solid var(--marker-strong);
  }

  .type-member-status {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 13px;
    line-height: 1.4;
    color: var(--ink-fade);
    margin: 0;
  }

  /* ── Spacing table ──────────────────────────────────────────────────────── */
  .spacing-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .spacing-table th {
    text-align: left;
    padding: 8px 12px;
    background: var(--surface-muted);
    color: var(--ink-fade);
    font-weight: 500;
    font-size: 11px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .spacing-table td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--rule-soft);
    vertical-align: middle;
  }

  .spacing-table td code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--brand-accent);
  }

  .spacing-bar {
    height: 8px;
    background: var(--marker);
    border-radius: 2px;
    max-width: 200px;
  }

  .ink-fade {
    color: var(--ink-fade);
  }

  /* ── Component sections ─────────────────────────────────────────────────── */
  .component-row {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    align-items: flex-start;
  }

  .component-group {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dot-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
