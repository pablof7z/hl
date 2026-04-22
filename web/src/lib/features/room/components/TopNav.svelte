<script lang="ts">
  import type { Snippet } from 'svelte';
  import LoginDialog from '$lib/features/auth/LoginDialog.svelte';

  type Variant = 'app' | 'marketing';

  let {
    variant = 'app',
    activeLink,
    right
  }: {
    variant?: Variant;
    activeLink?: string;
    right?: Snippet;
  } = $props();

  let loginOpen = $state(false);

  const APP_LINKS = [
    { href: '/rooms', label: 'Your rooms', key: 'rooms' },
    { href: '/discover', label: 'Discover', key: 'discover' },
    { href: '/vault', label: 'Vault', key: 'vault' }
  ];

  const MARKETING_LINKS = [
    { href: '#what', label: 'What it is', key: 'what' },
    { href: '#media', label: 'For everything', key: 'media' },
    { href: '#room', label: 'Rooms', key: 'room' },
    { href: '/discover', label: 'Discover', key: 'discover' }
  ];

  const links = $derived(variant === 'marketing' ? MARKETING_LINKS : APP_LINKS);
</script>

<nav class="topnav" class:marketing={variant === 'marketing'}>
  <div class="nav-left">
    <a href="/" class="nav-logo">Highlighter<em>.</em></a>
    <div class="nav-links">
      {#each links as link (link.key)}
        <a href={link.href} class:active={activeLink === link.key}>{link.label}</a>
      {/each}
    </div>
  </div>
  <div class="nav-right">
    {#if variant === 'marketing'}
      <button type="button" class="btn btn-ghost btn-sm" onclick={() => (loginOpen = true)}>Log in</button>
      <a href="/onboarding" class="nav-capture">Join</a>
      <LoginDialog showTrigger={false} bind:open={loginOpen} />
    {:else if right}
      {@render right()}
    {/if}
  </div>
</nav>

<style>
  .topnav {
    position: sticky;
    top: 0;
    background: var(--surface);
    border-bottom: 1px solid var(--rule);
    padding: 14px 28px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 20;
    height: 62px;
  }

  .topnav.marketing {
    background: var(--bg);
    padding: 18px 40px;
    height: auto;
  }

  .nav-left {
    display: flex;
    align-items: center;
    gap: 32px;
  }

  .nav-logo {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 17px;
    letter-spacing: -0.02em;
    color: var(--ink);
    text-decoration: none;
  }

  .topnav.marketing .nav-logo {
    font-family: var(--font-serif);
    font-weight: 500;
    font-size: 22px;
    letter-spacing: -0.01em;
  }

  .nav-logo em {
    font-style: normal;
    font-weight: 500;
    color: var(--brand-accent);
  }

  .topnav.marketing .nav-logo em {
    font-style: italic;
    font-weight: 400;
  }

  .nav-links {
    display: flex;
    gap: 22px;
    font-size: 13.5px;
    color: var(--ink-soft);
  }

  .topnav.marketing .nav-links {
    gap: 32px;
    font-size: 14px;
  }

  .nav-links a {
    text-decoration: none;
    padding: 6px 0;
    position: relative;
    font-weight: 500;
    color: inherit;
    transition: color 150ms ease;
  }

  .nav-links a.active {
    color: var(--ink);
  }

  .nav-links a.active::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    bottom: -16px;
    height: 2px;
    background: var(--brand-accent);
  }

  .nav-links a:hover {
    color: var(--ink);
  }

  .topnav.marketing .nav-links a:hover {
    color: var(--brand-accent);
  }

  .nav-right {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .nav-capture {
    padding: 8px 18px;
    background: var(--ink);
    color: var(--surface);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    letter-spacing: 0.01em;
    transition: background 200ms ease;
  }

  .nav-capture:hover {
    background: var(--brand-accent);
  }

  @media (max-width: 780px) {
    .topnav { padding: 14px 20px; }
    .topnav.marketing { padding: 14px 20px; }
    .nav-links { gap: 16px; font-size: 13px; }
    .nav-links a:not(.nav-capture) { display: initial; }
  }

  @media (max-width: 600px) {
    .nav-links {
      display: none;
    }
  }
</style>
