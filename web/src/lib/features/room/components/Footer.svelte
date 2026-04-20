<script lang="ts">
  type Variant = 'app' | 'room' | 'marketing';

  let {
    variant = 'app',
    roomMeta
  }: {
    variant?: Variant;
    roomMeta?: string;
  } = $props();
</script>

{#if variant === 'marketing'}
  <footer class="landing-footer">
    <div class="footer-inner">
      <div class="footer-mark">Highlighter<em>.</em></div>
      <div class="footer-cols">
        <div>
          <h6>The product</h6>
          <a href="#what">What it is</a>
          <a href="#media">For any medium</a>
          <a href="#room">Rooms</a>
        </div>
        <div>
          <h6>Read</h6>
          <a href="/about">How a room works</a>
          <a href="/about">On the Nostr protocol</a>
          <a href="/changelog">A note on the old Highlighter</a>
        </div>
        <div>
          <h6>Reach us</h6>
          <a href="/onboarding">Join</a>
          <a href="/onboarding">Bring a room</a>
          <a href="/about">contact</a>
        </div>
      </div>
    </div>
  </footer>
{:else}
  <footer class="room-footer">
    <div class="brand">Highlighter<em>.</em></div>
    {#if roomMeta}
      <div class="meta">{roomMeta}</div>
    {:else}
      <div class="meta">Built on Nostr · Your rooms, your data, always.</div>
    {/if}
  </footer>
{/if}

<style>
  /* ── App / room footer ─────────────────── */
  .room-footer {
    border-top: 1px solid var(--rule);
    padding: 32px var(--container-px);
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-fade);
    max-width: var(--container-max);
    margin: 0 auto;
    width: 100%;
  }

  .brand {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 14px;
    color: var(--ink);
    text-transform: none;
    letter-spacing: -0.01em;
  }

  .brand em {
    font-style: normal;
    color: var(--brand-accent);
  }

  @media (max-width: 600px) {
    .room-footer {
      flex-direction: column;
      gap: 12px;
      align-items: flex-start;
    }
  }

  /* ── Marketing footer ─────────────────── */
  .landing-footer {
    background: var(--surface-warm, #EFE9DC);
    border-top: 1px solid var(--rule);
    padding: 48px 40px;
    margin-top: 40px;
    font-family: var(--font-sans);
  }

  .footer-inner {
    max-width: var(--container-max);
    margin: 0 auto;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 32px;
  }

  .footer-mark {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 20px;
    color: var(--ink);
  }

  .footer-mark em {
    font-style: italic;
    color: var(--brand-accent);
  }

  .footer-cols {
    display: flex;
    gap: 48px;
    font-size: 13px;
    color: var(--ink-fade);
  }

  .footer-cols a {
    display: block;
    text-decoration: none;
    margin-bottom: 8px;
    color: inherit;
  }

  .footer-cols a:hover {
    color: var(--brand-accent);
  }

  .footer-cols h6 {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink);
    margin: 0 0 12px;
  }
</style>
