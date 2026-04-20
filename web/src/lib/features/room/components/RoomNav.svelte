<script lang="ts">
  interface NavSection {
    id: string;
    label: string;
    count?: number;
  }

  let {
    sections
  }: {
    sections: NavSection[];
  } = $props();

  let activeId = $state('');

  $effect(() => {
    activeId = sections[0]?.id ?? '';

    function onScroll() {
      const top = window.scrollY + 140;
      const blocks = sections
        .map((s) => document.getElementById(s.id))
        .filter((el): el is HTMLElement => !!el);
      if (blocks.length === 0) return;
      let current = blocks[0];
      for (const b of blocks) {
        if (b.offsetTop <= top) current = b;
      }
      activeId = current.id;
    }

    window.addEventListener('scroll', onScroll, { passive: true });
    onScroll();
    return () => window.removeEventListener('scroll', onScroll);
  });
</script>

<nav class="roomnav" aria-label="Room sections">
  <div class="roomnav-inner">
    {#each sections as section (section.id)}
      <a href="#{section.id}" class:active={activeId === section.id}>
        {section.label}
        {#if section.count !== undefined}
          <span class="count">{section.count}</span>
        {/if}
      </a>
    {/each}
  </div>
</nav>

<style>
  .roomnav {
    position: sticky;
    top: 62px;
    background: var(--bg);
    border-bottom: 1px solid var(--rule);
    z-index: 15;
    margin: 0 calc(var(--container-px) * -1);
    padding: 0 var(--container-px);
    overflow-x: auto;
  }

  .roomnav-inner {
    display: flex;
    gap: 0;
    max-width: var(--container-max);
    margin: 0 auto;
  }

  .roomnav a {
    padding: 14px 18px 12px;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--ink-fade);
    text-decoration: none;
    border-bottom: 2px solid transparent;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 7px;
    transition: color 150ms ease;
  }

  .roomnav a:first-child {
    padding-left: 0;
  }

  .roomnav a.active {
    color: var(--ink);
    border-bottom-color: var(--brand-accent);
  }

  .roomnav a:hover {
    color: var(--ink);
  }

  .count {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-fade);
    font-weight: 400;
    letter-spacing: 0.02em;
  }

  .roomnav a.active .count {
    color: var(--brand-accent);
  }
</style>
