<script lang="ts">
  // Dev-only probe for the NMP web bridge (GitHub #65, Slice 1).
  //
  // NOT linked from nav. Used by Playwright tests to assert data-* attributes.
  //
  // data-bridge-kind     = "worker" | "in_process_fallback"
  // data-runtime-status  = "running" | "ready" | "degraded:<reason>"
  // data-has-snapshot    = "true" | "false"

  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { getClient, type RuntimeSnapshot } from '$lib/nmp/client.svelte';

  let snapshot: RuntimeSnapshot | null = $state(null);
  let startError: string | null = $state(null);

  function formatStatus(s: RuntimeSnapshot['status']): string {
    if (typeof s === 'string') return s;
    return `degraded:${s.degraded}`;
  }

  onMount(() => {
    if (!browser) return;

    const client = getClient();

    // Subscribe so we always have the latest snapshot.
    const unsub = client.subscribe((snap) => {
      snapshot = snap;
    });

    // Send hello first, then start.
    client.hello();

    const relayBootstrapParam = new URL(window.location.href).searchParams.get('relay_bootstrap');
    let relayBootstrap: { url: string; role: string }[] = [];
    if (relayBootstrapParam) {
      try {
        const raw = JSON.parse(relayBootstrapParam) as [string, string][];
        relayBootstrap = raw.map(([url, role]) => ({ url, role }));
      } catch {
        // ignore malformed param
      }
    }

    client
      .start({ relay_bootstrap: relayBootstrap })
      .catch((err: unknown) => {
        startError = err instanceof Error ? err.message : String(err);
      });

    return unsub;
  });
</script>

<main
  class="nmp-probe"
  data-bridge-kind={snapshot?.bridgeKind ?? 'pending'}
  data-runtime-status={snapshot ? formatStatus(snapshot.status) : 'pending'}
  data-has-snapshot={snapshot?.latestUpdateBytes != null ? 'true' : 'false'}
>
  <h1>NMP Bridge Probe</h1>

  <dl>
    <dt>Bridge kind</dt>
    <dd>{snapshot?.bridgeKind ?? '(initialising…)'}</dd>

    <dt>Runtime status</dt>
    <dd>{snapshot ? formatStatus(snapshot.status) : '(initialising…)'}</dd>

    <dt>Has snapshot</dt>
    <dd>{snapshot?.latestUpdateBytes != null ? 'yes' : 'no'}</dd>
  </dl>

  {#if startError}
    <p class="error">Start error: {startError}</p>
  {/if}

  {#if snapshot?.events.length}
    <details>
      <summary>Events ({snapshot.events.length})</summary>
      <pre>{JSON.stringify(
        snapshot.events.map((e) =>
          e.type === 'update_bytes'
            ? { type: 'update_bytes', byteLength: e.bytes.byteLength }
            : e
        ),
        null,
        2
      )}</pre>
    </details>
  {/if}
</main>
