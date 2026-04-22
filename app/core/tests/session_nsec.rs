//! Runtime check that nsec login round-trips consistently: generate a
//! keypair, encode as nsec, hand it to login_nsec, and verify the returned
//! pubkey matches what we started with.

use highlighter_core::HighlighterCore;
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

#[test]
fn nsec_login_roundtrips_generated_key() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().expect("encode nsec");

    let (core, _tmp) = isolated_core();
    let user = core
        .login_nsec(nsec)
        .expect("login_nsec should accept the nsec we just produced");

    assert_eq!(user.pubkey, keys.public_key().to_hex());
    assert_eq!(user.npub, keys.public_key().to_bech32().unwrap());
    assert_eq!(user.pubkey.len(), 64);
}

#[test]
fn nsec_login_accepts_hex_secret_key() {
    let keys = Keys::generate();
    let hex = keys.secret_key().to_secret_hex();

    let (core, _tmp) = isolated_core();
    let user = core.login_nsec(hex).expect("login_nsec should accept a hex secret key");
    assert_eq!(user.pubkey, keys.public_key().to_hex());
}

#[test]
fn nsec_login_rejects_garbage() {
    let (core, _tmp) = isolated_core();
    assert!(core.login_nsec("not a real nsec".to_string()).is_err());
    assert!(core.login_nsec(String::new()).is_err());
    assert!(core.login_nsec("nsec1garbage".to_string()).is_err());
}

#[test]
fn current_user_reflects_login_state() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let (core, _tmp) = isolated_core();

    assert!(core.current_user().is_none());
    let _ = core.login_nsec(nsec).unwrap();
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
    let user = core.login_nsec(padded).expect("surrounding whitespace should be tolerated");
    assert_eq!(user.pubkey, keys.public_key().to_hex());
}
