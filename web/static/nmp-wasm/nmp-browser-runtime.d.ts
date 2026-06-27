/* tslint:disable */
/* eslint-disable */

/**
 * `NmpWasmRuntime` — the browser JS host constructs one instance per Worker,
 * then drives it with `handle_json` / `handle_dispatch_bytes`.
 *
 * Exported to JS as `new NmpWasmRuntime()`.
 */
export class NmpWasmRuntime {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Handle raw `DispatchEnvelope` bytes (ADR-0064 binary write doorway).
     *
     * Avoids JSON round-tripping the binary payload, which would corrupt it.
     * Returns a JSON array of `WorkerEvent`s (same as `handle_json`).
     */
    handle_dispatch_bytes(bytes: Uint8Array): any;
    /**
     * Handle a JSON-serialised `WorkerRequest` and return a JSON array
     * of `WorkerEvent`s.
     *
     * After the request runs, attempts to install any pending wake closure
     * (deferred from a `set_snapshot_callback` call that preceded `Start`),
     * then pushes the updated snapshot (#2139 BLOCKER 1).
     *
     * The return value is `unknown` from the JS side; the bridge casts it
     * via `parseWorkerEvents`. Large binary payloads should go through
     * `handle_dispatch_bytes` instead.
     */
    handle_json(request: string): any;
    /**
     * Construct an unstarted runtime.
     *
     * Call `handle_json` with a `WorkerRequest::Hello` then
     * `WorkerRequest::Start` before any other requests.
     */
    constructor();
    /**
     * Async pre-`Start` hook: open the durable OPFS-SQLite store and stash it
     * on the core so the next (synchronous) `Start` injects it instead of an
     * in-memory store (#1007 PR-7).
     *
     * The host MUST `await` this before sending `WorkerRequest::Start`. This
     * is the async-open-before-`Start` seam: `OpfsSqliteEventStore::open`
     * acquires the OPFS SyncAccessHandle pool asynchronously — work the
     * synchronous `handle_start` cannot do, so it is hoisted here and the
     * ready `Arc<dyn EventStore>` parked on the core for `handle_start` to
     * `take()` and `inject_store(..)`.
     *
     * `app_id` + `database_name` compose the per-app OPFS namespace (see
     * [`super::core::opfs_database_name`]).
     *
     * # Degraded-mode diagnostics (#1007 PR-8)
     *
     * On a successful open the ready `Arc<dyn EventStore>` is parked for
     * `handle_start` to inject. On **open failure** the error is classified
     * into a **stable reason string** ([`super::store_failure`]) and parked
     * on the core; `handle_start` threads it through
     * `BrowserAppBuilder::with_store_open_failure` so the in-memory fallback
     * session reports the **same** Tier-3 `store_open_failure` diagnostic
     * the native LMDB degraded-open path emits. Never a silent
     * pretend-durable: durability is OFF and the host sees exactly why
     * (Safari < 17.4, private browsing, quota, handle loss, second-tab
     * pool-lock).
     *
     * Gated on `feature = "opfs-sqlite-backend"`: a wasm build without the
     * durable backend simply has no such hook and starts in-memory.
     */
    prepare_store(app_id: string, database_name: string): Promise<void>;
    /**
     * Return a JSON snapshot of recent routing decisions (pull-only,
     * diagnostic). Does NOT trigger a snapshot push.
     */
    recent_routing_decisions(): string;
    /**
     * Install (or clear) the snapshot callback.
     *
     * The callback is called with a `Uint8Array` of merged FlatBuffers
     * update-frame bytes whenever the kernel state changes. Pass `null`
     * to uninstall.
     *
     * This wires the async pump wake: when inbound relay events arrive the
     * relay pool fires a 0ms timer that calls `pump_and_push_snapshot()` on
     * the shared inner state, which drains inbox + pushes snapshot without
     * holding the borrow at JS boundary.
     *
     * # Wake ordering fix (#2139 BLOCKER 1)
     *
     * `wasmBridge.ts` calls `set_snapshot_callback` in its constructor,
     * BEFORE the host sends `Start`. The handle does not exist yet, so the
     * wake closure cannot be installed immediately. Instead it is stored in
     * `pending_wake` and installed onto the handle the next time
     * `handle_json` (or `handle_dispatch_bytes`) is called after `Start`
     * creates the handle.
     */
    set_snapshot_callback(cb?: Function | null): void;
}

/**
 * Encode a 32-byte secp256k1 public key (64 hex chars) as a JSON object
 * `{"npub":"npub1…","npubShort":"npub1abc…xyz"}`.
 *
 * Returns the JSON string on success, or an empty string if `hex` is not
 * valid 64-char hex (D6: total on JS boundary — never throws).
 *
 * The bridge (`wasmBridge.ts` line 74) calls `JSON.parse(json)` expecting
 * exactly the `{npub, npubShort}` shape (#2139 BLOCKER 3 — was returning
 * a bare `npub1…` string which caused `JSON.parse` to throw on the object
 * destructure).
 *
 * Exported to JS as `nmp_encode_npub(hex: string): string`.
 */
export function nmp_encode_npub(hex: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_nmpwasmruntime_free: (a: number, b: number) => void;
    readonly nmp_encode_npub: (a: number, b: number) => [number, number];
    readonly nmpwasmruntime_handle_dispatch_bytes: (a: number, b: number, c: number) => any;
    readonly nmpwasmruntime_handle_json: (a: number, b: number, c: number) => any;
    readonly nmpwasmruntime_new: () => number;
    readonly nmpwasmruntime_prepare_store: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly nmpwasmruntime_recent_routing_decisions: (a: number) => [number, number];
    readonly nmpwasmruntime_set_snapshot_callback: (a: number, b: number) => void;
    readonly rustsecp256k1_v0_10_0_default_error_callback_fn: (a: number, b: number) => void;
    readonly rustsecp256k1_v0_10_0_default_illegal_callback_fn: (a: number, b: number) => void;
    readonly rustsecp256k1_v0_10_0_context_destroy: (a: number) => void;
    readonly rustsecp256k1_v0_10_0_context_create: (a: number) => number;
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___wasm_bindgen_5107baffd0a75d26___JsValue__core_996c9f5f00cf318b___result__Result_____wasm_bindgen_5107baffd0a75d26___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__2: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__3: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
