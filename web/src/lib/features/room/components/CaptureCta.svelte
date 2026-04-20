<script lang="ts">
  let {
    title = 'Share to this room',
    subtitle = 'Something everyone here should read.',
    href = '#'
  }: {
    title?: string;
    subtitle?: string;
    href?: string;
  } = $props();
</script>

<a {href} class="capture-cta">
  <div class="capture-cta-ico">✎</div>
  <div class="capture-cta-t">
    <b>{title}</b>
    <span>{subtitle}</span>
  </div>
</a>

<style>
  .capture-cta {
    background: var(--ink);
    color: var(--surface);
    padding: 18px 22px;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 12px;
    text-decoration: none;
    transition: background 200ms ease;
  }

  .capture-cta:hover {
    background: var(--brand-accent);
  }

  .capture-cta-ico {
    font-size: 20px;
    line-height: 1;
    font-family: var(--font-serif);
    font-style: italic;
  }

  .capture-cta-t {
    display: flex;
    flex-direction: column;
  }

  .capture-cta-t b {
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 14px;
    margin-bottom: 2px;
  }

  .capture-cta-t span {
    font-family: var(--font-sans);
    font-size: 13px;
    line-height: 1.4;
    opacity: 0.75;
  }
</style>
