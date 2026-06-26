//! Typed byte dispatch seam — ADR-0064 Cut-B replacement for the deleted
//! `nmp_app_dispatch_action` JSON doorway.
//!
//! All write actions that previously called `nmp_app_dispatch_action(namespace, json)`
//! now go through `dispatch_action_bytes_for`, which encodes the typed
//! `ActionPayload` bytes, wraps them in an open `DispatchEnvelope`, and calls
//! `nmp_app_dispatch_action_bytes`. No JSON crosses the FFI boundary.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string, NmpApp};

static NEXT_CORRELATION_ID: AtomicU64 = AtomicU64::new(1);

fn mint_correlation_id() -> String {
    let n = NEXT_CORRELATION_ID.fetch_add(1, Ordering::Relaxed);
    format!("hl-{n}")
}

fn encode_payload_for_namespace(namespace: &str, json: &str) -> Result<Vec<u8>, String> {
    match namespace {
        "nmp.follow" | "nmp.unfollow" => encode::<nmp_nip02::PubkeyAction>(namespace, json),
        "nmp.nip25.react" => encode::<nmp_nip25::ReactAction>(namespace, json),
        "nmp.nip25.unreact" => encode::<nmp_nip25::UnreactAction>(namespace, json),
        "nmp.nip29.discover" => encode::<nmp_nip29::action::DiscoverGroupsInput>(namespace, json),
        "nmp.nip29.join" => encode::<nmp_nip29::action::JoinGroupInput>(namespace, json),
        "nmp.nip29.create_public_group" => {
            encode::<nmp_nip29::action::CreatePublicGroupInput>(namespace, json)
        }
        "nmp.nip29.put_user" => encode::<nmp_nip29::action::PutUserInput>(namespace, json),
        "nmp.nip29.create_invite" => {
            encode::<nmp_nip29::action::CreateInviteInput>(namespace, json)
        }
        "nmp.nip29.repost_in_group" => {
            encode::<nmp_nip29::action::RepostInGroupInput>(namespace, json)
        }
        "nmp.nip29.share_event_in_group" => {
            encode::<nmp_nip29::action::ShareEventInGroupInput>(namespace, json)
        }
        "nmp.nip29.publish_group_event" => {
            encode::<nmp_nip29::action::PublishGroupEventInput>(namespace, json)
        }
        "nmp.publish" => encode::<nmp_core::publish::PublishAction>(namespace, json),
        "nmp.nip51.add_bookmark" | "nmp.nip51.remove_bookmark" => {
            encode::<nmp_nip51::BookmarkUpdateInput>(namespace, json)
        }
        "nmp.nip22.post_comment" => encode::<nmp_nip22::PostCommentAction>(namespace, json),
        "nmp.blossom.upload" => encode::<nmp_blossom::UploadInput>(namespace, json),
        other => Err(format!("no typed payload encoder for namespace '{other}'")),
    }
}

fn encode<P>(namespace: &str, json: &str) -> Result<Vec<u8>, String>
where
    P: ActionPayload + DeserializeOwned,
{
    let action: P = serde_json::from_str(json).map_err(|e| {
        format!("action body for '{namespace}' does not match typed payload shape: {e}")
    })?;
    Ok(action.encode())
}

/// Dispatch an hl action through the NMP typed byte doorway.
///
/// Encodes `json` into the typed `ActionPayload` bytes for `namespace`, wraps
/// them in a `DispatchEnvelope`, and calls `nmp_app_dispatch_action_bytes`.
/// Returns the echoed correlation_id on accept, or an error string on
/// rejection (D6).
pub(crate) fn dispatch_action_bytes_for(
    app: *mut NmpApp,
    namespace: &str,
    json: &str,
) -> Result<String, String> {
    if app.is_null() {
        return Err("runtime app is not available".to_string());
    }
    let payload = encode_payload_for_namespace(namespace, json)?;
    let correlation_id = mint_correlation_id();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload,
    );
    let ptr = nmp_app_dispatch_action_bytes(app, envelope.as_ptr(), envelope.len());
    if ptr.is_null() {
        return Err("action dispatch returned null".to_string());
    }
    let text = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    nmp_free_string(ptr);
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("action dispatch returned invalid JSON: {e}"))?;
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        return Err(error.to_string());
    }
    value
        .get("correlation_id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "action dispatch envelope missing correlation_id".to_string())
}
