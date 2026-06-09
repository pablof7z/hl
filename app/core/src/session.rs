//! NIP-46 bunker + nsec session management. UX patterns follow Olas iOS
//! (`Olas-iOS-60m1gj/OlasApp/Views/Auth/LoginView.swift`):
//!
//! - Swift does signer *detection* (`canOpenURL`) and UI for the Primal hero
//!   button. Rust is never responsible for probing installed apps — that's an
//!   iOS-only concern.
//! - Swift calls `start_default_nostr_connect()` on this module to produce an
//!   outgoing `nostrconnect://` URI and listen for the remote signer on the
//!   Primal relay.
//! - Swift calls `pair_bunker()` when the user pastes/scans a `bunker://` or
//!   `nostrconnect://` URI produced by a remote signer.
//! - Nsec persistence is Swift-side (iOS Keychain via `AppSessionStore`).
//!   The Rust core only holds the active `Keys` in memory for the life of
//!   the session.

use std::sync::Arc;

use nostr_sdk::prelude::*;

use crate::errors::CoreError;
use crate::models::{CurrentUser, GeneratedAccount, LoginInputAction};
use crate::nip46::BunkerSigner;

const COMPACT_NPUB_THRESHOLD: usize = 20;
const COMPACT_NPUB_PREFIX: usize = 10;
const COMPACT_NPUB_SUFFIX: usize = 8;
const MASKED_NSEC_THRESHOLD: usize = 10;
const MASKED_NSEC_PREFIX: usize = 8;
const MASKED_NSEC_SUFFIX: usize = 6;
const MASKED_NSEC_MIDDLE: &str = "••••••••••••••••••••••••";

pub fn classify_login_input(input: &str) -> LoginInputAction {
    let trimmed = input.trim();
    let normalized = trimmed.strip_prefix("nostr:").unwrap_or(trimmed);
    if normalized.is_empty() {
        return LoginInputAction::Empty;
    }

    if normalized.starts_with("nsec1") {
        LoginInputAction::Nsec {
            nsec: normalized.to_string(),
        }
    } else if normalized.starts_with("bunker://") || normalized.starts_with("nostrconnect://") {
        LoginInputAction::Bunker {
            uri: normalized.to_string(),
        }
    } else {
        LoginInputAction::Invalid {
            message: "Enter an nsec1… or bunker:// URI.".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AuthSessionSnapshot {
    pub user: Option<CurrentUser>,
    pub is_authenticated: bool,
    pub error_message: String,
}

pub fn auth_session_snapshot(result: Result<CurrentUser, CoreError>) -> AuthSessionSnapshot {
    match result {
        Ok(user) => AuthSessionSnapshot {
            user: Some(user),
            is_authenticated: true,
            error_message: String::new(),
        },
        Err(error) => AuthSessionSnapshot {
            user: None,
            is_authenticated: false,
            error_message: error.to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AuthSessionRestoreSnapshot {
    pub user: Option<CurrentUser>,
    pub is_authenticated: bool,
    pub error_message: String,
    pub clear_nsec: bool,
    pub clear_bunker_uri: bool,
}

pub fn auth_session_restore_snapshot(
    result: Result<Option<CurrentUser>, CoreError>,
    clear_nsec: bool,
    clear_bunker_uri: bool,
) -> AuthSessionRestoreSnapshot {
    match result {
        Ok(Some(user)) => AuthSessionRestoreSnapshot {
            user: Some(user),
            is_authenticated: true,
            error_message: String::new(),
            clear_nsec,
            clear_bunker_uri,
        },
        Ok(None) => AuthSessionRestoreSnapshot {
            user: None,
            is_authenticated: false,
            error_message: String::new(),
            clear_nsec,
            clear_bunker_uri,
        },
        Err(error) => AuthSessionRestoreSnapshot {
            user: None,
            is_authenticated: false,
            error_message: error.to_string(),
            clear_nsec,
            clear_bunker_uri,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountGenerationSnapshot {
    pub account: Option<GeneratedAccount>,
    pub succeeded: bool,
    pub error_message: String,
}

pub fn account_generation_snapshot(
    result: Result<GeneratedAccount, CoreError>,
) -> AccountGenerationSnapshot {
    match result {
        Ok(account) => AccountGenerationSnapshot {
            account: Some(account),
            succeeded: true,
            error_message: String::new(),
        },
        Err(error) => AccountGenerationSnapshot {
            account: None,
            succeeded: false,
            error_message: error.to_string(),
        },
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PublicKeyDisplayProjectionInput {
    pub npub: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PublicKeyDisplayProjection {
    pub compact_label: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SecretKeyDisplayProjectionInput {
    pub nsec: String,
    pub is_revealed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SecretKeyDisplayProjection {
    pub display_value: String,
}

/// Settings identity projection for the user's public NIP-19 key. Rust owns
/// compact key labeling so native shells do not duplicate identity formatting.
pub fn public_key_display_projection(
    input: PublicKeyDisplayProjectionInput,
) -> PublicKeyDisplayProjection {
    PublicKeyDisplayProjection {
        compact_label: compact_npub_label(&input.npub),
    }
}

/// Secret-key display projection. Native shells may own ephemeral reveal
/// toggles, but Rust owns how unrevealed identity material is masked.
pub fn secret_key_display_projection(
    input: SecretKeyDisplayProjectionInput,
) -> SecretKeyDisplayProjection {
    let display_value = if input.is_revealed {
        input.nsec
    } else {
        masked_nsec_label(&input.nsec)
    };
    SecretKeyDisplayProjection { display_value }
}

fn compact_npub_label(npub: &str) -> String {
    if npub.chars().count() <= COMPACT_NPUB_THRESHOLD {
        return npub.to_string();
    }

    let prefix: String = npub.chars().take(COMPACT_NPUB_PREFIX).collect();
    let suffix = trailing_chars(npub, COMPACT_NPUB_SUFFIX);
    format!("{prefix}…{suffix}")
}

fn masked_nsec_label(nsec: &str) -> String {
    let char_count = nsec.chars().count();
    if char_count <= MASKED_NSEC_THRESHOLD {
        return "•".repeat(char_count);
    }

    let prefix: String = nsec.chars().take(MASKED_NSEC_PREFIX).collect();
    let suffix = trailing_chars(nsec, MASKED_NSEC_SUFFIX);
    format!("{prefix}{MASKED_NSEC_MIDDLE}{suffix}")
}

fn trailing_chars(value: &str, count: usize) -> String {
    let mut suffix: Vec<char> = value.chars().rev().take(count).collect();
    suffix.reverse();
    suffix.into_iter().collect()
}

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
    /// Follows' kind:10002 (NIP-65 relay lists). Backfills the data the
    /// outbox planner needs to route the user's home feeds — without this
    /// every follow lands in the "uncovered" fallback shard. Long-lived;
    /// new relay-list publications by a follow keep landing in nostrdb.
    follows_nip65_subscription: Option<SubscriptionId>,
    /// Current user's *own* kind:10002 + kind:30078. Without this, a
    /// fresh install with cold cache stays on `seed_defaults()` forever
    /// even when the user's NIP-65 says they publish elsewhere. Long-
    /// lived so cross-device edits to the user's relay config land too.
    user_relay_config_subscription: Option<SubscriptionId>,
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
        self.follows_nip65_subscription = None;
        self.user_relay_config_subscription = None;
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

    pub fn has_follows_nip65_subscription(&self) -> bool {
        self.follows_nip65_subscription.is_some()
    }

    pub fn set_follows_nip65_subscription(&mut self, id: SubscriptionId) {
        self.follows_nip65_subscription = Some(id);
    }

    pub fn take_follows_nip65_subscription(&mut self) -> Option<SubscriptionId> {
        self.follows_nip65_subscription.take()
    }

    pub fn set_user_relay_config_subscription(&mut self, id: SubscriptionId) {
        self.user_relay_config_subscription = Some(id);
    }

    pub fn take_user_relay_config_subscription(&mut self) -> Option<SubscriptionId> {
        self.user_relay_config_subscription.take()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_login_input_strips_nostr_prefix_and_preserves_material() {
        assert_eq!(
            classify_login_input("  nostr:nsec1example  "),
            LoginInputAction::Nsec {
                nsec: "nsec1example".into()
            }
        );
        assert_eq!(
            classify_login_input("nostr:bunker://relay.example"),
            LoginInputAction::Bunker {
                uri: "bunker://relay.example".into()
            }
        );
        assert_eq!(classify_login_input(" nostr: "), LoginInputAction::Empty);
    }

    #[test]
    fn classify_login_input_rejects_unknown_material_with_login_message() {
        assert_eq!(
            classify_login_input("npub1example"),
            LoginInputAction::Invalid {
                message: "Enter an nsec1… or bunker:// URI.".into()
            }
        );
    }

    #[test]
    fn auth_session_snapshot_projects_success_and_error_states() {
        let user = CurrentUser {
            pubkey: "abc123".into(),
            npub: "npub1abc".into(),
        };
        let success = auth_session_snapshot(Ok(user.clone()));
        assert_eq!(success.user, Some(user));
        assert!(success.is_authenticated);
        assert!(success.error_message.is_empty());

        let failure = auth_session_snapshot(Err(CoreError::InvalidInput("bad key".into())));
        assert_eq!(failure.user, None);
        assert!(!failure.is_authenticated);
        assert_eq!(failure.error_message, "invalid input: bad key");
    }

    #[test]
    fn auth_session_restore_snapshot_projects_cleanup_policy() {
        let user = CurrentUser {
            pubkey: "abc123".into(),
            npub: "npub1abc".into(),
        };
        let success = auth_session_restore_snapshot(Ok(Some(user.clone())), true, false);
        assert_eq!(success.user, Some(user));
        assert!(success.is_authenticated);
        assert!(success.error_message.is_empty());
        assert!(success.clear_nsec);
        assert!(!success.clear_bunker_uri);

        let no_credentials = auth_session_restore_snapshot(Ok(None), false, false);
        assert_eq!(no_credentials.user, None);
        assert!(!no_credentials.is_authenticated);
        assert!(no_credentials.error_message.is_empty());
        assert!(!no_credentials.clear_nsec);
        assert!(!no_credentials.clear_bunker_uri);

        let failure = auth_session_restore_snapshot(
            Err(CoreError::InvalidInput("bad key".into())),
            true,
            true,
        );
        assert_eq!(failure.user, None);
        assert!(!failure.is_authenticated);
        assert_eq!(failure.error_message, "invalid input: bad key");
        assert!(failure.clear_nsec);
        assert!(failure.clear_bunker_uri);
    }

    #[test]
    fn account_generation_snapshot_projects_success_and_error_states() {
        let account = GeneratedAccount {
            user: CurrentUser {
                pubkey: "abc123".into(),
                npub: "npub1abc".into(),
            },
            nsec: "nsec1abc".into(),
        };
        let success = account_generation_snapshot(Ok(account.clone()));
        assert_eq!(success.account, Some(account));
        assert!(success.succeeded);
        assert!(success.error_message.is_empty());

        let failure = account_generation_snapshot(Err(CoreError::Other("entropy failed".into())));
        assert_eq!(failure.account, None);
        assert!(!failure.succeeded);
        assert_eq!(failure.error_message, "entropy failed");
    }

    #[test]
    fn public_key_display_projection_compacts_long_npubs() {
        let projection = public_key_display_projection(PublicKeyDisplayProjectionInput {
            npub: "npub1abcdefghijklmnopqrstuvwxyz".into(),
        });

        assert_eq!(projection.compact_label, "npub1abcde…stuvwxyz");
    }

    #[test]
    fn public_key_display_projection_leaves_short_npubs_unmodified() {
        let projection = public_key_display_projection(PublicKeyDisplayProjectionInput {
            npub: "npub1short".into(),
        });

        assert_eq!(projection.compact_label, "npub1short");
    }

    #[test]
    fn secret_key_display_projection_masks_hidden_nsec() {
        let projection = secret_key_display_projection(SecretKeyDisplayProjectionInput {
            nsec: "nsec1abcdefghijklmnopqrstuvwxyz".into(),
            is_revealed: false,
        });

        assert_eq!(
            projection.display_value,
            "nsec1abc••••••••••••••••••••••••uvwxyz"
        );
    }

    #[test]
    fn secret_key_display_projection_masks_short_nsec_by_length() {
        let projection = secret_key_display_projection(SecretKeyDisplayProjectionInput {
            nsec: "nsec1".into(),
            is_revealed: false,
        });

        assert_eq!(projection.display_value, "•••••");
    }

    #[test]
    fn secret_key_display_projection_reveals_raw_nsec() {
        let projection = secret_key_display_projection(SecretKeyDisplayProjectionInput {
            nsec: "nsec1abcdefghijklmnopqrstuvwxyz".into(),
            is_revealed: true,
        });

        assert_eq!(projection.display_value, "nsec1abcdefghijklmnopqrstuvwxyz");
    }
}
