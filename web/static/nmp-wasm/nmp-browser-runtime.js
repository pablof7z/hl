/* @ts-self-types="./nmp-browser-runtime.d.ts" */
import { bindBlob, bindInt64, bindNull, bindText, closeDb, columnBlob, columnInt64, columnText, exec, finalize, openDb, prepare, step } from './snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs';
import * as import1 from "./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs"
import * as import2 from "./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs"


/**
 * `NmpWasmRuntime` — the browser JS host constructs one instance per Worker,
 * then drives it with `handle_json` / `handle_dispatch_bytes`.
 *
 * Exported to JS as `new NmpWasmRuntime()`.
 */
export class NmpWasmRuntime {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NmpWasmRuntimeFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_nmpwasmruntime_free(ptr, 0);
    }
    /**
     * Handle raw `DispatchEnvelope` bytes (ADR-0064 binary write doorway).
     *
     * Avoids JSON round-tripping the binary payload, which would corrupt it.
     * Returns a JSON array of `WorkerEvent`s (same as `handle_json`).
     * @param {Uint8Array} bytes
     * @returns {any}
     */
    handle_dispatch_bytes(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.nmpwasmruntime_handle_dispatch_bytes(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
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
     * @param {string} request
     * @returns {any}
     */
    handle_json(request) {
        const ptr0 = passStringToWasm0(request, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.nmpwasmruntime_handle_json(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Construct an unstarted runtime.
     *
     * Call `handle_json` with a `WorkerRequest::Hello` then
     * `WorkerRequest::Start` before any other requests.
     */
    constructor() {
        const ret = wasm.nmpwasmruntime_new();
        this.__wbg_ptr = ret;
        NmpWasmRuntimeFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
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
     * @param {string} app_id
     * @param {string} database_name
     * @returns {Promise<void>}
     */
    prepare_store(app_id, database_name) {
        const ptr0 = passStringToWasm0(app_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(database_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.nmpwasmruntime_prepare_store(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Return a JSON snapshot of recent routing decisions (pull-only,
     * diagnostic). Does NOT trigger a snapshot push.
     * @returns {string}
     */
    recent_routing_decisions() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.nmpwasmruntime_recent_routing_decisions(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
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
     * @param {Function | null} [cb]
     */
    set_snapshot_callback(cb) {
        wasm.nmpwasmruntime_set_snapshot_callback(this.__wbg_ptr, isLikeNone(cb) ? 0 : addToExternrefTable0(cb));
    }
}
if (Symbol.dispose) NmpWasmRuntime.prototype[Symbol.dispose] = NmpWasmRuntime.prototype.free;

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
 * @param {string} hex
 * @returns {string}
 */
export function nmp_encode_npub(hex) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.nmp_encode_npub(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_0accd80f45e5faa2: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_754e9f305ff6029e: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_87c3bfe968c6a5ad: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_object_56732c2bc353f41d: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_string_c236cabd84a4d769: function(arg0) {
            const ret = typeof(arg0) === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_67b456be8673d3d7: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_string_get_72bdf95d3ae505b1: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_1506f2235d1bdba0: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_61db23ac97f16c31: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_bindBlob_c7b828d284223df5: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            bindBlob(arg0, arg1, getArrayU8FromWasm0(arg2, arg3));
        }, arguments); },
        __wbg_bindInt64_237ba917d35d6e2a: function() { return handleError(function (arg0, arg1, arg2) {
            bindInt64(arg0, arg1, arg2);
        }, arguments); },
        __wbg_bindNull_3cefb07d20558a78: function() { return handleError(function (arg0, arg1) {
            bindNull(arg0, arg1);
        }, arguments); },
        __wbg_bindText_c9f36f5e5e6dc485: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            bindText(arg0, arg1, getStringFromWasm0(arg2, arg3));
        }, arguments); },
        __wbg_call_9c758de292015997: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clearTimeout_113b1cde814ec762: function(arg0) {
            const ret = clearTimeout(arg0);
            return ret;
        },
        __wbg_closeDb_db3687b59bf14af5: function() { return handleError(function (arg0) {
            closeDb(arg0);
        }, arguments); },
        __wbg_columnBlob_78622d727dc23188: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = columnBlob(arg1, arg2);
            const ptr1 = passArray8ToWasm0(ret, wasm.__wbindgen_malloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_columnInt64_fbd9b2ced93bffc9: function() { return handleError(function (arg0, arg1) {
            const ret = columnInt64(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_columnText_353b8cd2c8d9a2ee: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = columnText(arg1, arg2);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_crypto_38df2bab126b63dc: function(arg0) {
            const ret = arg0.crypto;
            return ret;
        },
        __wbg_data_bd354b70c783c66e: function(arg0) {
            const ret = arg0.data;
            return ret;
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_exec_f7aed46b971a629b: function() { return handleError(function (arg0, arg1, arg2) {
            exec(arg0, getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_finalize_3a0821e01b010c13: function() { return handleError(function (arg0) {
            finalize(arg0);
        }, arguments); },
        __wbg_getRandomValues_c44a50d8cfdaebeb: function() { return handleError(function (arg0, arg1) {
            arg0.getRandomValues(arg1);
        }, arguments); },
        __wbg_get_de6a0f7d4d18a304: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_instanceof_ArrayBuffer_8f49811467741499: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Error_94c8c9d9e410014a: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Error;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Promise_d0db99486956c8e8: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Promise;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_e093be59ee9a8e14: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_length_4a591ecaa01354d9: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_message_40300ed2d1f8bdc6: function(arg0) {
            const ret = arg0.message;
            return ret;
        },
        __wbg_message_ab75609e36338e7c: function(arg0, arg1) {
            const ret = arg1.message;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_msCrypto_bd5a034af96bcba6: function(arg0) {
            const ret = arg0.msCrypto;
            return ret;
        },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_578aeef4b6b94378: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_ce1ab61c1c2b300d: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_d7e476b433a26bea: function() { return handleError(function (arg0, arg1) {
            const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_d90091b82fdf5b91: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_from_slice_18fa1f71286d66b8: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_typed_bf31d18f92484486: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_new_with_length_36a4998e27b014c5: function(arg0) {
            const ret = new Uint8Array(arg0 >>> 0);
            return ret;
        },
        __wbg_node_84ea875411254db1: function(arg0) {
            const ret = arg0.node;
            return ret;
        },
        __wbg_now_190933fa139cc119: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_now_e7c6795a7f81e10f: function(arg0) {
            const ret = arg0.now();
            return ret;
        },
        __wbg_openDb_aa07135026cdc2a5: function() { return handleError(function (arg0, arg1) {
            const ret = openDb(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_performance_3fcf6e32a7e1ed0a: function(arg0) {
            const ret = arg0.performance;
            return ret;
        },
        __wbg_prepare_617b14cc68b9c5f5: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = prepare(arg0, getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_process_44c7a14e11e9f69e: function(arg0) {
            const ret = arg0.process;
            return ret;
        },
        __wbg_prototypesetcall_3249fc62a0fafa30: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_push_a6822215aa43e71c: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_queueMicrotask_35c611f4a14830b2: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_queueMicrotask_404ed0a58e0b63cc: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_randomFillSync_6c25eac9869eb53c: function() { return handleError(function (arg0, arg1) {
            arg0.randomFillSync(arg1);
        }, arguments); },
        __wbg_readyState_490503c1fa8f8dd6: function(arg0) {
            const ret = arg0.readyState;
            return ret;
        },
        __wbg_reason_4624d424a130e5b2: function(arg0, arg1) {
            const ret = arg1.reason;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_require_b4edbdcf3e2a1ef0: function() { return handleError(function () {
            const ret = module.require;
            return ret;
        }, arguments); },
        __wbg_resolve_25a7e548d5881dca: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_send_35647f35f8bdac5d: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.send(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_setTimeout_ef24d2fc3ad97385: function() { return handleError(function (arg0, arg1) {
            const ret = setTimeout(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_set_6e30c9374c26414c: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_binaryType_41994c453b95bdd2: function(arg0, arg1) {
            arg0.binaryType = __wbindgen_enum_BinaryType[arg1];
        },
        __wbg_set_onclose_13787fb31ae8aefd: function(arg0, arg1) {
            arg0.onclose = arg1;
        },
        __wbg_set_onerror_5a45265839edf1b1: function(arg0, arg1) {
            arg0.onerror = arg1;
        },
        __wbg_set_onmessage_9c6b4cb14e244b7f: function(arg0, arg1) {
            arg0.onmessage = arg1;
        },
        __wbg_set_onopen_db452f4233e99d7d: function(arg0, arg1) {
            arg0.onopen = arg1;
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_static_accessor_GLOBAL_9d53f2689e622ca1: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_a1a35cec07001a8a: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_4c59f6c7ea29a144: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_e70ae9f2eb052253: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_step_02d663ce2005e802: function() { return handleError(function (arg0) {
            const ret = step(arg0);
            return ret;
        }, arguments); },
        __wbg_stringify_8286df6dcc591521: function() { return handleError(function (arg0) {
            const ret = JSON.stringify(arg0);
            return ret;
        }, arguments); },
        __wbg_subarray_4aa221f6a4f5ab22: function(arg0, arg1, arg2) {
            const ret = arg0.subarray(arg1 >>> 0, arg2 >>> 0);
            return ret;
        },
        __wbg_then_18f476d590e58992: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_ac7b025999b52837: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_versions_276b2795b1c6a219: function(arg0) {
            const ret = arg0.versions;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 1888, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___wasm_bindgen_5107baffd0a75d26___JsValue__core_996c9f5f00cf318b___result__Result_____wasm_bindgen_5107baffd0a75d26___JsError___true_);
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 1706, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("ErrorEvent")], shim_idx: 1706, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__2);
            return ret;
        },
        __wbindgen_cast_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 1706, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__3);
            return ret;
        },
        __wbindgen_cast_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 1704, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke_______true_);
            return ret;
        },
        __wbindgen_cast_0000000000000006: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000007: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_0000000000000008: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./nmp-browser-runtime_bg.js": import0,
        "./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs": import1,
        "./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs": import2,
    };
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke_______true_(arg0, arg1) {
    wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke_______true_(arg0, arg1);
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true_(arg0, arg1, arg2);
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__2(arg0, arg1, arg2) {
    wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__2(arg0, arg1, arg2);
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2) {
    wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___web_sys_2222d82dbcf7556e___features__gen_CloseEvent__CloseEvent______true__3(arg0, arg1, arg2);
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___wasm_bindgen_5107baffd0a75d26___JsValue__core_996c9f5f00cf318b___result__Result_____wasm_bindgen_5107baffd0a75d26___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___wasm_bindgen_5107baffd0a75d26___JsValue__core_996c9f5f00cf318b___result__Result_____wasm_bindgen_5107baffd0a75d26___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_5107baffd0a75d26___convert__closures_____invoke___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined___js_sys_994cffaf55f38238___Function_fn_wasm_bindgen_5107baffd0a75d26___JsValue_____wasm_bindgen_5107baffd0a75d26___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_BinaryType = ["blob", "arraybuffer"];
const NmpWasmRuntimeFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_nmpwasmruntime_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('nmp-browser-runtime_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
