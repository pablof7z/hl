//! NIP-46 bunker + nsec session management. UX patterns follow Olas iOS
//! (`Olas-iOS-60m1gj/OlasApp/Views/Auth/LoginView.swift`):
//!
//! - Swift does signer *detection* (`canOpenURL`) and UI for the Primal hero
//!   button. Rust is never responsible for probing installed apps — that's an
//!   iOS-only concern.
//! - Swift calls `start_nostr_connect()` on this module to produce an outgoing
//!   `nostrconnect://` URI and listen for the remote signer on the Primal
//!   relay.
//! - Swift calls `pair_bunker()` when the user pastes/scans a `bunker://` or
//!   `nostrconnect://` URI produced by a remote signer.
//! - Nsec persistence is Swift-side (iOS Keychain via `AppSessionStore`).
//!   The Rust core only holds the active `Keys` in memory for the life of
//!   the session.

use std::sync::Arc;

use nostr_sdk::prelude::*;

use crate::errors::CoreError;
use crate::models::CurrentUser;
use crate::nip46::BunkerSigner;

#[derive(Default)]
pub struct Session {
    signer: Option<ActiveSigner>,
    /// Subscription id for the global post-login membership feed. Retained
    /// so `logout` can drop it. None when logged out.
    membership_subscription: Option<SubscriptionId>,
    /// Subscription id for the logged-in user's own kind:3 contact list —
    /// installed so `is_following(...)` answers instantly without a relay
    /// roundtrip on first follow/unfollow.
    contacts_subscription: Option<SubscriptionId>,
    /// Rooms explorer catalog: one long-lived relay sub pulling every
    /// kind:39000 metadata event. Installed on first explorer appearance,
    /// kept until logout so subsequent appearances are instant.
    discovery_subscription: Option<SubscriptionId>,
    /// Curated-list sub: kind:10012 authored by the configured curator
    /// (relay.highlighter.com's pubkey). Installed on first explorer
    /// appearance, same lifecycle as `discovery_subscription`.
    curation_subscription: Option<SubscriptionId>,
    /// Friends' memberships sub: kind:39001/39002 where any of the user's
    /// follows appears in a `p` tag. Powers the "Friends are here" shelf by
    /// dragging non-own-room membership events into the local cache.
    /// Installed on first explorer appearance.
    friends_memberships_subscription: Option<SubscriptionId>,
    /// Friends' groups-list sub: kind:10009 authored by any follow — the
    /// denser half of the Friends-are-here signal. User-owned, broadcast
    /// publicly, so more reliable than the relay-owned 39002.
    friends_groups_list_subscription: Option<SubscriptionId>,
}

enum ActiveSigner {
    Nsec(Keys),
    /// NIP-46 remote signer. The `user` pubkey is cached because
    /// `BunkerSigner::get_public_key` is async and `current_user()` must not
    /// block. The `signer` handle is retained for its lifecycle: keeping the
    /// Arc alive in Session prevents the relay subscription task from being
    /// dropped out from under the `nostr_sdk::Client` while the app still
    /// uses it (set_signer takes its own reference too, but Session owns the
    /// canonical handle for logout).
    Bunker {
        #[allow(dead_code)]
        signer: Arc<BunkerSigner>,
        user: CurrentUser,
    },
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn login_nsec(&mut self, nsec: &str) -> Result<CurrentUser, CoreError> {
        let trimmed = nsec.trim();
        let keys = Keys::parse(trimmed)
            .map_err(|e| CoreError::InvalidInput(format!("invalid nsec: {e}")))?;
        let user = current_user_from_pubkey(&keys.public_key())?;
        self.signer = Some(ActiveSigner::Nsec(keys));
        Ok(user)
    }

    /// Install a NIP-46 signer that's already completed its handshake.
    pub fn set_bunker(&mut self, signer: Arc<BunkerSigner>, user: CurrentUser) {
        self.signer = Some(ActiveSigner::Bunker { signer, user });
    }

    pub fn logout(&mut self) {
        self.signer = None;
        self.membership_subscription = None;
        self.contacts_subscription = None;
        self.discovery_subscription = None;
        self.curation_subscription = None;
        self.friends_memberships_subscription = None;
        self.friends_groups_list_subscription = None;
    }

    pub fn set_membership_subscription(&mut self, id: SubscriptionId) {
        self.membership_subscription = Some(id);
    }

    pub fn take_membership_subscription(&mut self) -> Option<SubscriptionId> {
        self.membership_subscription.take()
    }

    pub fn set_contacts_subscription(&mut self, id: SubscriptionId) {
        self.contacts_subscription = Some(id);
    }

    pub fn take_contacts_subscription(&mut self) -> Option<SubscriptionId> {
        self.contacts_subscription.take()
    }

    pub fn has_discovery_subscription(&self) -> bool {
        self.discovery_subscription.is_some()
    }

    pub fn set_discovery_subscription(&mut self, id: SubscriptionId) {
        self.discovery_subscription = Some(id);
    }

    pub fn take_discovery_subscription(&mut self) -> Option<SubscriptionId> {
        self.discovery_subscription.take()
    }

    pub fn has_curation_subscription(&self) -> bool {
        self.curation_subscription.is_some()
    }

    pub fn set_curation_subscription(&mut self, id: SubscriptionId) {
        self.curation_subscription = Some(id);
    }

    pub fn take_curation_subscription(&mut self) -> Option<SubscriptionId> {
        self.curation_subscription.take()
    }

    pub fn has_friends_memberships_subscription(&self) -> bool {
        self.friends_memberships_subscription.is_some()
    }

    pub fn set_friends_memberships_subscription(&mut self, id: SubscriptionId) {
        self.friends_memberships_subscription = Some(id);
    }

    pub fn take_friends_memberships_subscription(&mut self) -> Option<SubscriptionId> {
        self.friends_memberships_subscription.take()
    }

    pub fn has_friends_groups_list_subscription(&self) -> bool {
        self.friends_groups_list_subscription.is_some()
    }

    pub fn set_friends_groups_list_subscription(&mut self, id: SubscriptionId) {
        self.friends_groups_list_subscription = Some(id);
    }

    pub fn take_friends_groups_list_subscription(&mut self) -> Option<SubscriptionId> {
        self.friends_groups_list_subscription.take()
    }

    pub fn current_user(&self) -> Option<CurrentUser> {
        match &self.signer {
            Some(ActiveSigner::Nsec(keys)) => current_user_from_pubkey(&keys.public_key()).ok(),
            Some(ActiveSigner::Bunker { user, .. }) => Some(user.clone()),
            None => None,
        }
    }

    /// Exposed so feature modules (publishing, subscriptions) can obtain an
    /// NDK-ready signing interface without this module knowing about them.
    pub fn keys(&self) -> Option<&Keys> {
        match &self.signer {
            Some(ActiveSigner::Nsec(keys)) => Some(keys),
            _ => None,
        }
    }

    /// Pubkey of the currently-active signer, regardless of type. Cheap — no
    /// relay roundtrip for NIP-46.
    pub fn pubkey(&self) -> Option<PublicKey> {
        match &self.signer {
            Some(ActiveSigner::Nsec(keys)) => Some(keys.public_key()),
            Some(ActiveSigner::Bunker { user, .. }) => PublicKey::from_hex(&user.pubkey).ok(),
            None => None,
        }
    }
}

pub(crate) fn current_user_from_pubkey(pk: &PublicKey) -> Result<CurrentUser, CoreError> {
    let npub = pk
        .to_bech32()
        .map_err(|e| CoreError::Other(format!("npub encoding failed: {e}")))?;
    Ok(CurrentUser {
        pubkey: pk.to_hex(),
        npub,
    })
}
