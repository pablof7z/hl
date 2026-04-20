<script lang="ts">
  let {
    roomMeta
  }: {
    roomMeta?: string;
  } = $props();
</script>

<footer class="room-footer">
  <div class="brand">Highlighter<em>.</em></div>
  {#if roomMeta}
    <div class="meta">{roomMeta}</div>
  {/if}
</footer>

<style>
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
</style>
