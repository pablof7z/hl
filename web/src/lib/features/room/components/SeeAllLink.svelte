<script lang="ts">
  let {
    label,
    href
  }: {
    label: string;
    href: string;
  } = $props();
</script>

<a {href} class="see-all">{label}</a>

<style>
  .see-all {
    margin-top: 20px;
    text-align: center;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
    display: block;
    padding: 14px;
    border: 1px dashed var(--rule);
    border-radius: var(--radius);
    transition: all 150ms ease;
  }

  .see-all::after {
    content: ' →';
  }

  .see-all:hover {
    border-color: var(--brand-accent);
    border-style: solid;
    background: var(--surface);
  }
</style>
