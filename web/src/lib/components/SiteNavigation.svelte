<script lang="ts">
  import { page } from '$app/state';
  import { ndk } from '$lib/ndk/client';

  const currentUser = $derived(ndk.$currentUser);

  const publicItems = [
    { href: '/discover', label: 'Discover' },
    { href: '/community', label: 'Communities' }
  ];

  function isActive(href: string): boolean {
    const pathname = page.url.pathname;
    if (href === '/discover') return pathname === '/discover';
    if (href === '/community') return pathname === '/community' || pathname.startsWith('/community/');
    if (href === '/me') return pathname === '/me' || pathname.startsWith('/me/');
    return pathname === href || pathname.startsWith(`${href}/`);
  }
</script>

<nav class="site-navigation">
  {#each publicItems as item (item.href)}
    <a
      href={item.href}
      class="site-navigation-link"
      class:active={isActive(item.href)}
      aria-current={isActive(item.href) ? 'page' : undefined}
    >
      {item.label}
    </a>
  {/each}

  {#if currentUser}
    <a
      href="/me"
      class="site-navigation-link"
      class:active={isActive('/me')}
      aria-current={isActive('/me') ? 'page' : undefined}
    >
      Me
    </a>
  {/if}
</nav>
