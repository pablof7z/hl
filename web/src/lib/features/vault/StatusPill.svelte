<script lang="ts">
  let {
    label,
    tone = 'needs-teaser'
  }: {
    label: string;
    tone?: 'ready' | 'needs-teaser' | 'already-shared';
  } = $props();
</script>

<span class={`status-pill ${tone}`}>{label}</span>

<style>
  .status-pill {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, currentColor 18%, transparent);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .status-pill.ready {
    background: color-mix(in srgb, #8a9a7f 16%, white);
    color: #6d7b63;
  }

  .status-pill.needs-teaser {
    background: var(--surface-soft);
    color: var(--muted);
  }

  .status-pill.already-shared {
    background: color-mix(in srgb, #1f6c9f 14%, white);
    color: #1f6c9f;
  }
</style>
