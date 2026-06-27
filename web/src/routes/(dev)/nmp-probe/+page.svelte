<script lang="ts">
  // Dev-only probe for the NMP web bridge (GitHub #65, Slices 1–3).
  //
  // NOT linked from nav. Used by Playwright tests to assert data-* attributes.
  //
  // Slice 1 attributes:
  //   data-bridge-kind     = "worker" | "in_process_fallback" | "pending"
  //   data-runtime-status  = "running" | "ready" | "degraded:<reason>" | "pending"
  //   data-has-snapshot    = "true" | "false"
  //
  // Slice 2 signer attributes (S2, #65):
  //   data-identity-set    = "true" | "false" | "pending"
  //     Set once setSigner resolves with action_accepted (no capability_failure).
  //     Mirrors ?set_identity_pubkey= query param.
  //   data-sign-result     = "completed:<correlationId>" | "failed:<reason>" | "pending"
  //     Set once a sign_completed or sign_failed event arrives, when beginSign
  //     was triggered via ?begin_sign= query param.
  //
  // Slice 3 read-proof attributes (S3, #65):
  //   data-projection-keys = comma-separated list of projection keys from the first
  //     decoded UpdateFrame, or "" until a frame arrives.
  //   data-resolved-profile-displayname = display name of the resolved profile
  //     (set once refs.profile sidecar carries it), or "" if not yet resolved.
  //     Mirrors ?resolve_profile=<pubkeyHex> query param.
  //
  // Slice 2 query params (mirrors ?relay_bootstrap= pattern):
  //   ?set_identity_pubkey=<hex>          — install NIP-07 identity via setSigner
  //   ?begin_sign=<url-encoded-json>      — start a sign round-trip after setSigner
  //
  // Slice 3 query params:
  //   ?relay_bootstrap=<json>             — [[url, role], ...] relay bootstrap list
  //   ?resolve_profile=<pubkeyHex>        — resolve a profile by pubkey after start
  //   ?resolve_profile_hint=<relayUrl>    — relay URL hint for the resolve_ref call

  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { getClient, type RuntimeSnapshot, type WorkerEvent } from '$lib/nmp/client.svelte';

  // Namespace / shape / liveness codes (mirror Lane D FFI — see actions.ts in chirp).
  const REF_NS_PROFILE = 0;
  const REF_SHAPE_PROFILE_REF = 0;
  const REF_LIVENESS_LIVE = 1;

  let snapshot: RuntimeSnapshot | null = $state(null);
  let startError: string | null = $state(null);
  // Signer state (S2)
  let identitySet: boolean | null = $state(null); // null = pending
  let signResult: string | null = $state(null);   // null = pending (or no begin_sign param)
  // S3: resolved profile display name (null = not yet resolved / no param)
  let resolvedDisplayName: string | null = $state(null);

  function formatStatus(s: RuntimeSnapshot['status']): string {
    if (typeof s === 'string') return s;
    return `degraded:${s.degraded}`;
  }

  /** Return the most recent sign event from the events array (newest-first). */
  function latestSignEvent(events: WorkerEvent[]): WorkerEvent | undefined {
    return events.find((e) => e.type === 'sign_completed' || e.type === 'sign_failed');
  }

  onMount(() => {
    if (!browser) return;

    const client = getClient();
    const params = new URL(window.location.href).searchParams;

    // S3: ?resolve_profile param — the pubkey we want to resolve.
    const resolveProfileParam = params.get('resolve_profile');
    // Optional relay URL hint for the resolve_ref call (speeds up discovery).
    const resolveProfileHint = params.get('resolve_profile_hint');
    // Consumer ID scoped to this probe mount — released on unmount.
    const CONSUMER_ID = 'nmp-probe-s3';

    // Subscribe so we always have the latest snapshot. Also watch sign events and
    // resolved profiles.
    const unsub = client.subscribe((snap) => {
      snapshot = snap;
      // Update sign result from the event stream (only if begin_sign was requested).
      if (signResult === null || signResult === 'pending') {
        const signEv = latestSignEvent(snap.events);
        if (signEv?.type === 'sign_completed') {
          signResult = `completed:${signEv.correlation_id}`;
        } else if (signEv?.type === 'sign_failed') {
          signResult = `failed:${signEv.reason}`;
        }
      }
      // S3: check if the resolved profile has arrived in the store.
      if (resolveProfileParam && resolvedDisplayName === null) {
        const profile = client.resolveProfile(resolveProfileParam);
        if (profile?.displayName) {
          resolvedDisplayName = profile.displayName;
        }
      }
    });

    // Send hello first, then start.
    client.hello();

    const relayBootstrapParam = params.get('relay_bootstrap');
    let relayBootstrap: { url: string; role: string }[] = [];
    if (relayBootstrapParam) {
      try {
        const raw = JSON.parse(relayBootstrapParam) as [string, string][];
        relayBootstrap = raw.map(([url, role]) => ({ url, role }));
      } catch {
        // ignore malformed param
      }
    }

    // ─── S2: signer params ───────────────────────────────────────────────────
    const identityPubkeyParam = params.get('set_identity_pubkey');
    const beginSignParam = params.get('begin_sign');

    client
      .start({ relay_bootstrap: relayBootstrap })
      .then(async () => {
        // After runtime is started, install the NIP-07 identity if requested.
        if (identityPubkeyParam) {
          const snap = await client.setSigner(identityPubkeyParam);
          // action_accepted on the most recent event means identity was installed.
          const last = snap.events[0];
          identitySet = last?.type === 'action_accepted';
          // If begin_sign was also requested, fire the round-trip after identity set.
          if (identitySet && beginSignParam) {
            signResult = 'pending';
            // beginSign is fire-and-forget; result arrives via subscription.
            client.beginSign(identityPubkeyParam, decodeURIComponent(beginSignParam));
          }
        }

        // ─── S3: resolve profile ────────────────────────────────────────────
        if (resolveProfileParam) {
          // resolveRef is fire-and-forget from the probe's perspective: the
          // subscription above polls `client.resolveProfile()` on each snapshot.
          // Pass the optional relay hint so the wasm can bypass NIP-65 discovery.
          const hints = resolveProfileHint ? [resolveProfileHint] : [];
          await client.resolveRef(
            REF_NS_PROFILE,
            resolveProfileParam,
            REF_SHAPE_PROFILE_REF,
            REF_LIVENESS_LIVE,
            CONSUMER_ID,
            hints,
          );
        }
      })
      .catch((err: unknown) => {
        startError = err instanceof Error ? err.message : String(err);
      });

    return () => {
      // Release the ref on unmount (consumer-id discipline, ADR-0063).
      if (resolveProfileParam) {
        void client.releaseRef(REF_NS_PROFILE, resolveProfileParam, CONSUMER_ID);
      }
      unsub();
    };
  });

  // Derived data-* values for non-signer attrs
  const bridgeKind = $derived(snapshot?.bridgeKind ?? 'pending');
  const runtimeStatus = $derived(snapshot ? formatStatus(snapshot.status) : 'pending');
  const hasSnapshot = $derived(snapshot?.latestUpdateBytes != null ? 'true' : 'false');
  // Signer attrs
  const dataIdentitySet = $derived(
    identitySet === null ? 'pending' : identitySet ? 'true' : 'false'
  );
  const dataSignResult = $derived(signResult ?? 'pending');
  // S3 attrs
  const dataProjectionKeys = $derived(
    (snapshot?.projectionKeys ?? []).join(',')
  );
  const dataResolvedProfileDisplayname = $derived(resolvedDisplayName ?? '');
</script>

<main
  class="nmp-probe"
  data-bridge-kind={bridgeKind}
  data-runtime-status={runtimeStatus}
  data-has-snapshot={hasSnapshot}
  data-identity-set={dataIdentitySet}
  data-sign-result={dataSignResult}
  data-projection-keys={dataProjectionKeys}
  data-resolved-profile-displayname={dataResolvedProfileDisplayname}
>
  <h1>NMP Bridge Probe</h1>

  <dl>
    <dt>Bridge kind</dt>
    <dd>{snapshot?.bridgeKind ?? '(initialising…)'}</dd>

    <dt>Runtime status</dt>
    <dd>{snapshot ? formatStatus(snapshot.status) : '(initialising…)'}</dd>

    <dt>Has snapshot</dt>
    <dd>{snapshot?.latestUpdateBytes != null ? 'yes' : 'no'}</dd>

    <dt>Identity set</dt>
    <dd>{dataIdentitySet}</dd>

    <dt>Sign result</dt>
    <dd>{dataSignResult}</dd>

    <dt>Projection keys</dt>
    <dd>{dataProjectionKeys || '(none yet)'}</dd>

    <dt>Resolved profile display name</dt>
    <dd>{dataResolvedProfileDisplayname || '(none yet)'}</dd>
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
