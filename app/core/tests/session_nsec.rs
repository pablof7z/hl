//! Runtime check that nsec login round-trips consistently: generate a
//! keypair, encode as nsec, hand it to login_nsec, and verify the returned
//! pubkey matches what we started with.

use highlighter_core::{
    AuthSessionRestoreSnapshot, AuthSessionSnapshot, CurrentUser, HighlighterCore,
};
use nostr_sdk::prelude::*;
use std::sync::Arc;
use tempfile::TempDir;

/// Build a HighlighterCore with an isolated nostrdb dir so the test suite
/// doesn't write to the real application data directory.
fn isolated_core() -> (Arc<HighlighterCore>, TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
    (core, tmp)
}

fn expect_user(outcome: AuthSessionSnapshot) -> CurrentUser {
    assert!(
        outcome.is_authenticated,
        "login_nsec: {}",
        outcome.error_message
    );
    outcome.user.expect("login_nsec returned no user")
}

fn expect_restored_user(outcome: AuthSessionRestoreSnapshot) -> CurrentUser {
    assert!(
        outcome.is_authenticated,
        "restore_session_snapshot: {}",
        outcome.error_message
    );
    outcome
        .user
        .expect("restore_session_snapshot returned no user")
}

#[test]
fn nsec_login_roundtrips_generated_key() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().expect("encode nsec");

    let (core, _tmp) = isolated_core();
    let user = expect_user(core.login_nsec(nsec));

    assert_eq!(user.pubkey, keys.public_key().to_hex());
    assert_eq!(user.npub, keys.public_key().to_bech32().unwrap());
    assert_eq!(user.pubkey.len(), 64);
}

#[test]
fn nsec_login_accepts_hex_secret_key() {
    let keys = Keys::generate();
    let hex = keys.secret_key().to_secret_hex();

    let (core, _tmp) = isolated_core();
    let user = expect_user(core.login_nsec(hex));
    assert_eq!(user.pubkey, keys.public_key().to_hex());
}

#[test]
fn nsec_login_rejects_garbage() {
    let (core, _tmp) = isolated_core();
    assert!(!core
        .login_nsec("not a real nsec".to_string())
        .error_message
        .is_empty());
    assert!(!core.login_nsec(String::new()).error_message.is_empty());
    assert!(!core
        .login_nsec("nsec1garbage".to_string())
        .error_message
        .is_empty());
}

#[test]
fn current_user_reflects_login_state() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let (core, _tmp) = isolated_core();

    assert!(core.current_user().is_none());
    let _ = expect_user(core.login_nsec(nsec));
    let user = core.current_user().expect("current_user after login");
    assert_eq!(user.pubkey, keys.public_key().to_hex());

    core.logout();
    assert!(core.current_user().is_none());
}

#[test]
fn nsec_login_trims_surrounding_whitespace() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let padded = format!("  {nsec}\n");

    let (core, _tmp) = isolated_core();
    let user = expect_user(core.login_nsec(padded));
    assert_eq!(user.pubkey, keys.public_key().to_hex());
}

#[tokio::test]
async fn restore_session_with_no_credentials_is_idle() {
    let (core, _tmp) = isolated_core();
    let snapshot = core.restore_session_snapshot(None, None).await;

    assert!(snapshot.user.is_none());
    assert!(!snapshot.is_authenticated);
    assert!(snapshot.error_message.is_empty());
    assert!(!snapshot.clear_nsec);
    assert!(!snapshot.clear_bunker_uri);
}

#[tokio::test]
async fn restore_session_uses_valid_nsec_without_cleanup() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();

    let (core, _tmp) = isolated_core();
    let user = expect_restored_user(core.restore_session_snapshot(Some(nsec), None).await);

    assert_eq!(user.pubkey, keys.public_key().to_hex());
    assert_eq!(
        core.current_user().expect("current user").pubkey,
        user.pubkey
    );
}

#[tokio::test]
async fn restore_session_clears_invalid_nsec() {
    let (core, _tmp) = isolated_core();
    let snapshot = core
        .restore_session_snapshot(Some("not a real nsec".into()), None)
        .await;

    assert!(snapshot.user.is_none());
    assert!(!snapshot.is_authenticated);
    assert!(!snapshot.error_message.is_empty());
    assert!(snapshot.clear_nsec);
    assert!(!snapshot.clear_bunker_uri);
}
