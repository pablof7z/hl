<script lang="ts">
  type Variant = 'dark' | 'red' | 'blue' | 'green' | 'plum';

  let {
    title,
    author,
    kicker,
    variant = 'dark',
    bookmark = true
  }: {
    title: string;
    author?: string;
    kicker?: string;
    variant?: Variant;
    bookmark?: boolean;
  } = $props();
</script>

<div class="book-cover-lg {variant}">
  {#if bookmark}<div class="bc-bookmark"></div>{/if}
  {#if kicker}<div class="bc-top">{kicker}</div>{/if}
  <div class="bc-content">
    <div class="bc-title">{title}</div>
    {#if author}<div class="bc-author">{author}</div>{/if}
  </div>
</div>

<style>
  .book-cover-lg {
    aspect-ratio: 2/3;
    border-radius: 2px;
    padding: 22px 18px;
    color: #E6D9BC;
    font-family: var(--font-serif);
    font-weight: 400;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    box-shadow: 3px 3px 14px rgba(0, 0, 0, 0.18);
    position: relative;
  }

  .book-cover-lg.dark {
    background: linear-gradient(140deg, #1C1814 0%, #3A2D1E 55%, #1C1814 100%);
    color: #E6D9BC;
  }
  .book-cover-lg.blue {
    background: linear-gradient(140deg, #141C2E 0%, #1C2D4D 55%, #141C2E 100%);
    color: #D6DBE6;
  }
  .book-cover-lg.red {
    background: linear-gradient(140deg, #3A1818 0%, #5A2A2A 55%, #3A1818 100%);
    color: #E8D0C8;
  }
  .book-cover-lg.green {
    background: linear-gradient(140deg, #14241A 0%, #1E3A28 55%, #14241A 100%);
    color: #D2E0D4;
  }
  .book-cover-lg.plum {
    background: linear-gradient(140deg, #241422 0%, #3A1E35 55%, #241422 100%);
    color: #E0D0DA;
  }

  .book-cover-lg::before {
    content: '';
    position: absolute;
    top: 10px;
    bottom: 10px;
    left: 5px;
    width: 2px;
    background: rgba(230, 217, 188, 0.22);
  }

  .bc-bookmark {
    position: absolute;
    top: -4px;
    right: 16px;
    width: 14px;
    height: 30px;
    background: var(--brand-accent);
  }

  .bc-top {
    font-style: italic;
    font-size: 10px;
    opacity: 0.7;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .bc-content {
    margin-top: auto;
  }

  .bc-title {
    font-size: 18px;
    line-height: 1.08;
    margin-bottom: 10px;
    font-weight: 500;
    white-space: pre-line;
  }

  .bc-author {
    font-style: italic;
    font-size: 11px;
    opacity: 0.85;
    border-top: 1px solid rgba(230, 217, 188, 0.3);
    padding-top: 6px;
  }
</style>
