<script lang="ts">
  let {
    activeLink = 'rooms',
    userInitials = 'PF',
    userColorIndex = 2
  }: {
    activeLink?: 'rooms' | 'discover' | 'vault';
    userInitials?: string;
    userColorIndex?: number;
  } = $props();
</script>

<nav class="topnav">
  <div class="nav-left">
    <a href="/" class="nav-logo">Highlighter<em>.</em></a>
    <div class="nav-links">
      <a href="/rooms" class:active={activeLink === 'rooms'}>Your rooms</a>
      <a href="/discover" class:active={activeLink === 'discover'}>Discover</a>
      <a href="/vault" class:active={activeLink === 'vault'}>Vault</a>
    </div>
  </div>
  <div class="nav-right">
    <a href="/search" class="nav-icon-btn" title="Search" aria-label="Search">⌕</a>
    <a href="/share" class="nav-capture">Share to a room</a>
    <a href="/me" class="nav-avatar member-dot-{userColorIndex}" title="Account">{userInitials}</a>
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

  .nav-logo em {
    font-style: normal;
    font-weight: 500;
    color: var(--brand-accent);
  }

  .nav-links {
    display: flex;
    gap: 22px;
    font-size: 13.5px;
    color: var(--ink-soft);
  }

  .nav-links a {
    text-decoration: none;
    padding: 6px 0;
    position: relative;
    font-weight: 500;
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

  .nav-right {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .nav-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border: 1px solid var(--rule);
    background: var(--surface);
    color: var(--ink-soft);
    text-decoration: none;
    font-size: 15px;
    transition: border-color var(--transition), color var(--transition);
  }

  .nav-icon-btn:hover {
    border-color: var(--brand-accent);
    color: var(--brand-accent);
  }

  .nav-capture {
    padding: 9px 16px;
    background: var(--ink);
    color: var(--surface);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    display: flex;
    align-items: center;
    gap: 8px;
    transition: background var(--transition);
  }

  .nav-capture:hover {
    background: var(--brand-accent);
  }

  .nav-capture::before {
    content: '✎';
    font-size: 13px;
  }

  .nav-avatar {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    text-align: center;
    line-height: 34px;
    color: var(--ink);
    cursor: pointer;
    text-decoration: none;
    display: block;
  }

  .member-dot-1 { background: var(--h-amber); }
  .member-dot-2 { background: var(--h-sage); }
  .member-dot-3 { background: var(--h-rose); }
  .member-dot-4 { background: var(--h-blue); }
  .member-dot-5 { background: var(--h-lilac); }
  .member-dot-6 { background: var(--h-amber-l); }

  @media (max-width: 760px) {
    .nav-links {
      display: none;
    }
    .nav-capture {
      display: none;
    }
  }
</style>
