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
use nmp_native_runtime::NmpApp;

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
        "nmp.nip29.join" => encode::<nmp_nip29::action::JoinGroupInput>(namespace, json),
        "nmp.nip29.create_group" => encode::<nmp_nip29::action::CreateGroupInput>(namespace, json),
        "nmp.nip29.put_user" => encode::<nmp_nip29::action::PutUserInput>(namespace, json),
        "nmp.nip29.create_invite" => {
            encode::<nmp_nip29::action::CreateInviteInput>(namespace, json)
        }
        "nmp.nip29.publish_group_event" => {
            encode::<nmp_nip29::action::PublishGroupEventInput>(namespace, json)
        }
        "nmp.publish" => encode::<nmp_core::publish::PublishAction>(namespace, json),
        "nmp.nip51.add_bookmark" | "nmp.nip51.remove_bookmark" => {
            encode::<nmp_nip51::BookmarkUpdateInput>(namespace, json)
        }
        "nmp.replies.reply" => encode::<nmp_replies::ReplyAction>(namespace, json),
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
/// them in a `DispatchEnvelope`, and calls `nmp_uniffi_support::dispatch_action_vec`.
/// Returns the echoed correlation_id on accept, or an error string on
/// rejection (D6).
pub(crate) fn dispatch_action_bytes_for(
    app: &NmpApp,
    namespace: &str,
    json: &str,
) -> Result<String, String> {
    let payload = encode_payload_for_namespace(namespace, json)?;
    let correlation_id = mint_correlation_id();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload,
    );
    let outcome = nmp_uniffi_support::dispatch_action_vec(app, envelope);
    if let Some(error) = outcome.error {
        return Err(error);
    }
    outcome
        .correlation_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "action dispatch envelope missing correlation_id".to_string())
}
