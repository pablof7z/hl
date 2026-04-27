//! NIP-46 Nostr Connect client, hand-rolled on top of `nostr`'s nip46
//! primitives and nostr-sdk's relay pool. Mirrors the structure of the
//! `nostr-connect` crate's `NostrConnect` client but is scoped tight to the
//! two flows we actually need:
//!
//! - [`BunkerSigner::pair`] (incoming): user pastes `bunker://…` or
//!   `nostrconnect://…` → we connect, call `connect` + `get_public_key`.
//! - [`BunkerSigner::await_inbound`] (outgoing): we publish a
//!   `nostrconnect://…` URI and wait for a remote signer to send us a
//!   `connect` request; we ACK and derive the user pubkey from the sender.
//!
//! The resulting [`BunkerSigner`] implements [`NostrSigner`] so it can be
//! installed on `nostr_sdk::Client` for all subsequent event signing.

use std::sync::Arc;
use std::time::Duration;

use nostr_sdk::prelude::*;
use parking_lot::Mutex;
use tokio::sync::OnceCell;

use crate::errors::CoreError;

/// How long we wait for a NIP-46 request / response turnaround.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// How long we wait for an inbound `nostrconnect://` remote-signer handshake.
const INBOUND_TIMEOUT: Duration = Duration::from_secs(300);

/// A NIP-46 remote signer tied to a specific relay and remote pubkey.
///
/// The signer is cheap to clone (all shared state is `Arc`-backed) and is
/// installed on the `nostr_sdk::Client` via `IntoNostrSigner`.
#[derive(Debug, Clone)]
pub struct BunkerSigner {
    client: Client,
    local_keys: Keys,
    remote_signer_pubkey: PublicKey,
    /// User pubkey as reported by `get_public_key`; cached on first pair.
    user_pubkey: Arc<OnceCell<PublicKey>>,
    /// Optional secret from the original URI for the initial `connect`.
    secret: Arc<Mutex<Option<String>>>,
}

impl BunkerSigner {
    /// Ingest a `bunker://` or `nostrconnect://` URI the user pasted. Establishes
    /// the relay connection, sends `connect` + `get_public_key`, and returns a
    /// fully paired signer plus the resolved user pubkey.
    pub async fn pair(client: Client, uri_str: &str) -> Result<(Self, PublicKey), CoreError> {
        let uri = NostrConnectURI::parse(uri_str)
            .map_err(|e| CoreError::InvalidInput(format!("invalid NIP-46 URI: {e}")))?;

        let relays: Vec<RelayUrl> = uri.relays().to_vec();
        if relays.is_empty() {
            return Err(CoreError::InvalidInput(
                "NIP-46 URI missing relay".into(),
            ));
        }

        // `bunker://` URIs embed the remote signer pubkey directly. For
        // `nostrconnect://` we'd have to *wait* for an inbound `connect`
        // request — that flow is covered by [`BunkerSigner::await_inbound`],
        // not `pair`. Pasting a client URI and expecting pair to work is a
        // user error.
        let remote_signer_pubkey = *uri.remote_signer_public_key().ok_or_else(|| {
            CoreError::InvalidInput(
                "expected bunker:// URI with remote signer pubkey".into(),
            )
        })?;

        let local_keys = Keys::generate();

        for relay in &relays {
            if let Err(e) = client.add_relay(relay.as_str()).await {
                tracing::warn!(relay = %relay, error = %e, "add_relay");
            }
        }
        client.connect().await;

        let signer = Self {
            client: client.clone(),
            local_keys,
            remote_signer_pubkey,
            user_pubkey: Arc::new(OnceCell::new()),
            secret: Arc::new(Mutex::new(uri.secret().map(|s| s.to_string()))),
        };

        // Subscribe for responses scoped to our local pubkey. The sub stays
        // open for the life of the signer — we can't drop it or sign_event
        // later will never hear back.
        signer.install_response_subscription().await?;

        // Send `connect` + `get_public_key`. Both of these must succeed for
        // the pairing to be considered complete.
        signer.rpc_connect().await?;
        let user = signer.rpc_get_public_key().await?;
        let _ = signer.user_pubkey.set(user);

        Ok((signer, user))
    }

    /// Listen on `relay` for an incoming remote-signer `connect` request aimed
    /// at `local_keys`'s pubkey. On receipt, ACK, then resolve the user pubkey.
    ///
    /// Used by `start_nostr_connect`: we publish a `nostrconnect://` URI with
    /// our local pubkey; a signer (e.g. Primal) sees it and reaches out over
    /// the same relay.
    pub async fn await_inbound(
        client: Client,
        local_keys: Keys,
        expected_secret: Option<String>,
    ) -> Result<(Self, PublicKey), CoreError> {
        let mut notifications = client.notifications();

        // `.since(now)` instead of `.limit(0)` so the relay replays anything
        // that arrived during a backgrounding gap. With `limit=0` the relay
        // would treat reconnect-after-iOS-suspend as "from now on" and the
        // signer's `connect` event — sent while we were backgrounded after
        // the user tapped OK in Primal — would be lost forever.
        let filter = Filter::new()
            .kind(Kind::NostrConnect)
            .pubkey(local_keys.public_key())
            .since(Timestamp::now());

        let sub_id = SubscriptionId::generate();
        client
            .subscribe_with_id(sub_id.clone(), filter, None)
            .await
            .map_err(|e| CoreError::Relay(format!("subscribe nostrconnect: {e}")))?;

        let deadline = tokio::time::Instant::now() + INBOUND_TIMEOUT;

        loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| CoreError::Signer("inbound NIP-46 pairing timed out".into()))?;

            let notif = tokio::time::timeout(remaining, notifications.recv())
                .await
                .map_err(|_| CoreError::Signer("inbound NIP-46 pairing timed out".into()))?
                .map_err(|e| CoreError::Signer(format!("notification channel: {e}")))?;

            let RelayPoolNotification::Event { event, .. } = notif else {
                continue;
            };
            if event.kind != Kind::NostrConnect {
                continue;
            }

            let decrypted = match nip44::decrypt(
                local_keys.secret_key(),
                &event.pubkey,
                event.content.as_str(),
            ) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Ok(msg) = NostrConnectMessage::from_json(&decrypted) else {
                continue;
            };

            // Per NIP-46 nostrconnect:// flow, the signer replies with a
            // Response whose `result` echoes the secret we put in the URI
            // (or "ack"). We do NOT receive a `connect` Request here — that's
            // the bunker:// flow where the *client* initiates. Confirmed
            // against Olas/NDKSwift `NDKBunkerSigner.handleResponse` and
            // Primal's on-the-wire behavior (kind:24133 with
            // `{"id":..,"result":"<secret>"}`).
            let NostrConnectMessage::Response { result, error, .. } = msg else {
                continue;
            };

            if let Some(err) = error {
                return Err(CoreError::Signer(format!(
                    "signer rejected pairing: {err}"
                )));
            }

            let result_str = result.unwrap_or_default();
            let secret_matches = match &expected_secret {
                Some(expected) => result_str == *expected || result_str == "ack",
                None => true,
            };
            if !secret_matches {
                return Err(CoreError::Signer(format!(
                    "remote signer presented wrong secret: result={result_str}"
                )));
            }

            // The signer's response IS the ack — no return event to send.
            // The signer pubkey is the event author. Resolve user pubkey via
            // get_public_key on the same subscription (kept open below).
            let signer = Self {
                client: client.clone(),
                local_keys: local_keys.clone(),
                remote_signer_pubkey: event.pubkey,
                user_pubkey: Arc::new(OnceCell::new()),
                secret: Arc::new(Mutex::new(None)),
            };
            let user = signer.rpc_get_public_key().await?;
            let _ = signer.user_pubkey.set(user);

            // Keep the subscription open — it's the same sub all future
            // sign_event responses will arrive on.
            return Ok((signer, user));
        }
    }

    /// Cached user pubkey resolved at pair-time.
    pub fn user_pubkey(&self) -> Option<PublicKey> {
        self.user_pubkey.get().copied()
    }

    async fn install_response_subscription(&self) -> Result<(), CoreError> {
        // See `await_inbound`: `.since(now)` instead of `.limit(0)` so that
        // sign_event responses delivered while iOS had us suspended get
        // replayed on the resub after foregrounding.
        let filter = Filter::new()
            .kind(Kind::NostrConnect)
            .pubkey(self.local_keys.public_key())
            .since(Timestamp::now());
        let sub_id = SubscriptionId::generate();
        self.client
            .subscribe_with_id(sub_id, filter, None)
            .await
            .map_err(|e| CoreError::Relay(format!("subscribe nip46 responses: {e}")))?;
        Ok(())
    }

    async fn rpc_connect(&self) -> Result<(), CoreError> {
        let secret = self.secret.lock().take();
        let req = NostrConnectRequest::Connect {
            remote_signer_public_key: self.remote_signer_pubkey,
            secret,
        };
        let res = self.send_request(req).await?;
        res.to_ack()
            .map_err(|e| CoreError::Signer(format!("connect rejected: {e}")))
    }

    async fn rpc_get_public_key(&self) -> Result<PublicKey, CoreError> {
        let res = self.send_request(NostrConnectRequest::GetPublicKey).await?;
        res.to_get_public_key()
            .map_err(|e| CoreError::Signer(format!("get_public_key failed: {e}")))
    }

    async fn send_request(
        &self,
        req: NostrConnectRequest,
    ) -> Result<ResponseResult, CoreError> {
        let msg = NostrConnectMessage::request(&req);
        let req_id = msg.id().to_string();
        let method = req.method();

        let event = EventBuilder::nostr_connect(
            &self.local_keys,
            self.remote_signer_pubkey,
            msg,
        )
        .map_err(|e| CoreError::Signer(format!("build nip46 event: {e}")))?
        .sign_with_keys(&self.local_keys)
        .map_err(|e| CoreError::Signer(format!("sign nip46 event: {e}")))?;

        let mut notifications = self.client.notifications();

        self.client
            .send_event(&event)
            .await
            .map_err(|e| CoreError::Relay(format!("send nip46 request: {e}")))?;

        let deadline = tokio::time::Instant::now() + REQUEST_TIMEOUT;
        loop {
            let remaining = deadline
                .checked_duration_since(tokio::time::Instant::now())
                .ok_or_else(|| CoreError::Signer("nip46 request timed out".into()))?;
            let notif = tokio::time::timeout(remaining, notifications.recv())
                .await
                .map_err(|_| CoreError::Signer("nip46 request timed out".into()))?
                .map_err(|e| CoreError::Signer(format!("notification: {e}")))?;

            let RelayPoolNotification::Event { event, .. } = notif else {
                continue;
            };
            if event.kind != Kind::NostrConnect || event.pubkey != self.remote_signer_pubkey {
                continue;
            }

            let Ok(plaintext) = nip44::decrypt(
                self.local_keys.secret_key(),
                &event.pubkey,
                event.content.as_str(),
            ) else {
                continue;
            };
            let Ok(msg) = NostrConnectMessage::from_json(&plaintext) else {
                continue;
            };

            if msg.id() != req_id || !msg.is_response() {
                continue;
            }

            let response = msg
                .to_response(method)
                .map_err(|e| CoreError::Signer(format!("parse nip46 response: {e}")))?;
            if let Some(err) = response.error {
                if response.result.as_ref().is_some_and(|r| r.is_auth_url()) {
                    // `auth_url` flows aren't plumbed through — signer wants
                    // the user to open a browser to approve. For our iOS
                    // usage (Primal in-app approval) this shouldn't happen;
                    // surface it as a clear error so the user knows to
                    // approve in the signer app.
                    return Err(CoreError::Signer(format!(
                        "remote signer requires approval at {err}"
                    )));
                }
                return Err(CoreError::Signer(err));
            }
            return response
                .result
                .ok_or_else(|| CoreError::Signer("empty nip46 response".into()));
        }
    }

    async fn sign_unsigned(&self, unsigned: UnsignedEvent) -> Result<Event, CoreError> {
        let res = self
            .send_request(NostrConnectRequest::SignEvent(unsigned))
            .await?;
        res.to_sign_event()
            .map_err(|e| CoreError::Signer(format!("sign_event: {e}")))
    }

    async fn nip04_encrypt(
        &self,
        peer: PublicKey,
        text: String,
    ) -> Result<String, CoreError> {
        let res = self
            .send_request(NostrConnectRequest::Nip04Encrypt {
                public_key: peer,
                text,
            })
            .await?;
        res.to_nip04_encrypt()
            .map_err(|e| CoreError::Signer(format!("nip04_encrypt: {e}")))
    }

    async fn nip04_decrypt(
        &self,
        peer: PublicKey,
        ciphertext: String,
    ) -> Result<String, CoreError> {
        let res = self
            .send_request(NostrConnectRequest::Nip04Decrypt {
                public_key: peer,
                ciphertext,
            })
            .await?;
        res.to_nip04_decrypt()
            .map_err(|e| CoreError::Signer(format!("nip04_decrypt: {e}")))
    }

    async fn nip44_encrypt_req(
        &self,
        peer: PublicKey,
        text: String,
    ) -> Result<String, CoreError> {
        let res = self
            .send_request(NostrConnectRequest::Nip44Encrypt {
                public_key: peer,
                text,
            })
            .await?;
        res.to_nip44_encrypt()
            .map_err(|e| CoreError::Signer(format!("nip44_encrypt: {e}")))
    }

    async fn nip44_decrypt_req(
        &self,
        peer: PublicKey,
        ciphertext: String,
    ) -> Result<String, CoreError> {
        let res = self
            .send_request(NostrConnectRequest::Nip44Decrypt {
                public_key: peer,
                ciphertext,
            })
            .await?;
        res.to_nip44_decrypt()
            .map_err(|e| CoreError::Signer(format!("nip44_decrypt: {e}")))
    }
}

impl NostrSigner for BunkerSigner {
    fn backend(&self) -> SignerBackend<'_> {
        SignerBackend::NostrConnect
    }

    fn get_public_key(&self) -> BoxedFuture<'_, Result<PublicKey, SignerError>> {
        Box::pin(async move {
            if let Some(pk) = self.user_pubkey.get().copied() {
                return Ok(pk);
            }
            self.rpc_get_public_key()
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }

    fn sign_event(&self, unsigned: UnsignedEvent) -> BoxedFuture<'_, Result<Event, SignerError>> {
        Box::pin(async move {
            self.sign_unsigned(unsigned)
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }

    fn nip04_encrypt<'a>(
        &'a self,
        public_key: &'a PublicKey,
        content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            self.nip04_encrypt(*public_key, content.to_string())
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }

    fn nip04_decrypt<'a>(
        &'a self,
        public_key: &'a PublicKey,
        content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            self.nip04_decrypt(*public_key, content.to_string())
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }

    fn nip44_encrypt<'a>(
        &'a self,
        public_key: &'a PublicKey,
        content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            self.nip44_encrypt_req(*public_key, content.to_string())
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }

    fn nip44_decrypt<'a>(
        &'a self,
        public_key: &'a PublicKey,
        content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            self.nip44_decrypt_req(*public_key, content.to_string())
                .await
                .map_err(|e| SignerError::from(e.to_string()))
        })
    }
}

/// Build the outgoing `nostrconnect://<local>?…` URI that we show as a QR to
/// the remote signer. The URI embeds the relay, app metadata, requested perms,
/// and a random secret the signer must echo back in its `connect` RPC.
pub fn build_nostr_connect_uri(
    local_public_key: PublicKey,
    relay: &str,
    app_name: &str,
    app_url: &str,
    app_image: &str,
    perms: &str,
    secret: &str,
) -> Result<String, CoreError> {
    let relay = RelayUrl::parse(relay)
        .map_err(|e| CoreError::InvalidInput(format!("nostrconnect relay: {e}")))?;

    // Build the `nostrconnect://` URI by hand rather than using
    // `NostrConnectURI::Client` + `Display` because the Display impl encodes
    // metadata as a single JSON blob (`metadata={…}`), whereas remote signers
    // in practice (Primal, nsec.app, Amber) expect flat `name=`, `url=`,
    // `image=`, `perms=`, `secret=` keys alongside `relay=`. Flat keys match
    // the NIP-46 spec's "informational parameters" section and what Olas
    // produces on iOS.
    let mut query = String::new();
    query.push_str("relay=");
    query.push_str(relay.as_str_without_trailing_slash());
    push_param(&mut query, "secret", secret);
    push_param(&mut query, "perms", perms);
    push_param(&mut query, "name", app_name);
    if !app_url.is_empty() {
        push_param(&mut query, "url", app_url);
    }
    if !app_image.is_empty() {
        push_param(&mut query, "image", app_image);
    }

    Ok(format!(
        "nostrconnect://{}?{}",
        local_public_key.to_hex(),
        query
    ))
}

fn push_param(query: &mut String, key: &str, value: &str) {
    query.push('&');
    query.push_str(key);
    query.push('=');
    query.push_str(&percent_encode(value));
}

/// Minimal percent-encoder for query values. Encodes anything outside the
/// unreserved set (RFC 3986) plus `&` and `=` to be safe.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        let keep = matches!(b,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' |
            b'-' | b'_' | b'.' | b'~' | b',' | b':' | b'/'
        );
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Generate a 16-byte hex secret for the `nostrconnect://` URI. Remote
/// signers echo this back in the `connect` RPC so we can verify the peer is
/// the one we handed the URI to.
pub fn random_secret() -> String {
    use secp256k1::rand::{rngs::OsRng, RngCore};
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_uri_has_flat_params_matching_spec() {
        let keys = Keys::generate();
        let uri = build_nostr_connect_uri(
            keys.public_key(),
            "wss://relay.primal.net",
            "Highlighter",
            "https://highlighter.com",
            "https://highlighter.com/icon.png",
            "sign_event:11,nip44_encrypt",
            "deadbeef",
        )
        .expect("build uri");

        assert!(uri.starts_with("nostrconnect://"));
        assert!(uri.contains(&keys.public_key().to_hex()));
        assert!(uri.contains("relay=wss://relay.primal.net"));
        assert!(uri.contains("secret=deadbeef"));
        assert!(uri.contains("perms=sign_event:11,nip44_encrypt"));
        assert!(uri.contains("name=Highlighter"));
    }

    #[test]
    fn random_secret_is_32_hex_chars() {
        let s = random_secret();
        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
