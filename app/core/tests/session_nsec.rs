//! Runtime check that nsec login round-trips through the exported NMP app
//! surface. The internal core is deliberately not part of the public contract.

use highlighter_core::{
    HighlighterAppAction, HighlighterAppConfig, HighlighterAppReconciler, HighlighterAppState,
    HighlighterNmpApp, HighlighterSessionCredential, HighlighterToastKind,
};
use nostr_sdk::prelude::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[derive(Debug, Clone)]
enum TestUpdate {
    State(HighlighterAppState),
    PersistSessionCredential,
    ClearSessionCredentials,
    OpenExternalUrl,
}

struct TestReconciler {
    tx: Sender<TestUpdate>,
}

impl HighlighterAppReconciler for TestReconciler {
    fn on_state(&self, state: HighlighterAppState) {
        let _ = self.tx.send(TestUpdate::State(state));
    }

    fn on_persist_session_credential(&self, _credential: HighlighterSessionCredential) {
        let _ = self.tx.send(TestUpdate::PersistSessionCredential);
    }

    fn on_clear_session_credentials(&self) {
        let _ = self.tx.send(TestUpdate::ClearSessionCredentials);
    }

    fn on_open_external_url(&self, _url: String) {
        let _ = self.tx.send(TestUpdate::OpenExternalUrl);
    }
}

fn isolated_app() -> (Arc<HighlighterNmpApp>, Receiver<TestUpdate>, TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = HighlighterNmpApp::new(HighlighterAppConfig {
        data_dir: Some(tmp.path().join("ndb").to_string_lossy().into_owned()),
        visible_limit: 8,
        emit_hz: 30,
        relay_policy_json: None,
    });
    let (tx, rx) = channel();
    app.listen_for_updates(Arc::new(TestReconciler { tx }));
    let _ = next_state(&rx);
    (app, rx, tmp)
}

fn next_state(rx: &Receiver<TestUpdate>) -> HighlighterAppState {
    for _ in 0..16 {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("app update within timeout")
        {
            TestUpdate::State(state) => return state,
            TestUpdate::PersistSessionCredential
            | TestUpdate::ClearSessionCredentials
            | TestUpdate::OpenExternalUrl => {}
        }
    }
    panic!("state update within timeout")
}

fn sign_in(
    app: &HighlighterNmpApp,
    rx: &Receiver<TestUpdate>,
    nsec: String,
) -> HighlighterAppState {
    app.dispatch(HighlighterAppAction::SignInNsec {
        nsec,
        persist: false,
        clear_stored_on_failure: false,
    });
    for _ in 0..16 {
        let state = next_state(rx);
        if state.chrome.current_user.is_some() || state.toast.is_some() {
            return state;
        }
    }
    panic!("terminal sign-in state")
}

#[test]
fn nsec_login_roundtrips_generated_key() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().expect("encode nsec");

    let (app, rx, _tmp) = isolated_app();
    let state = sign_in(&app, &rx, nsec);
    let user = state
        .chrome
        .current_user
        .expect("sign-in should publish a current user snapshot");

    assert_eq!(user.pubkey, keys.public_key().to_hex());
    assert_eq!(user.npub, keys.public_key().to_bech32().unwrap());
    assert_eq!(user.pubkey.len(), 64);
}

#[test]
fn nsec_login_accepts_hex_secret_key() {
    let keys = Keys::generate();
    let hex = keys.secret_key().to_secret_hex();

    let (app, rx, _tmp) = isolated_app();
    let state = sign_in(&app, &rx, hex);
    let user = state
        .chrome
        .current_user
        .expect("hex secret key should sign in");

    assert_eq!(user.pubkey, keys.public_key().to_hex());
}

#[test]
fn nsec_login_rejects_garbage() {
    for bad in ["not a real nsec", "", "nsec1garbage"] {
        let (app, rx, _tmp) = isolated_app();
        let state = sign_in(&app, &rx, bad.to_string());
        assert!(state.chrome.current_user.is_none());
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
    }
}

#[test]
fn current_user_reflects_login_state() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let (app, rx, _tmp) = isolated_app();

    assert!(app.state().chrome.current_user.is_none());
    let state = sign_in(&app, &rx, nsec);
    let user = state.chrome.current_user.expect("current_user after login");
    assert_eq!(user.pubkey, keys.public_key().to_hex());

    app.dispatch(HighlighterAppAction::Logout);
    // Sign-in resolutions are async (OpRunner): late pre-logout emissions
    // (e.g. the SignerConnected -> RefreshAppChrome chain) may still be
    // queued, so poll until the logout snapshot lands rather than asserting
    // on the first state after the dispatch.
    let mut logged_out = false;
    for _ in 0..16 {
        if next_state(&rx).chrome.current_user.is_none() {
            logged_out = true;
            break;
        }
    }
    assert!(logged_out, "logout snapshot must land");
}

#[test]
fn nsec_login_trims_surrounding_whitespace() {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().unwrap();
    let padded = format!("  {nsec}\n");

    let (app, rx, _tmp) = isolated_app();
    let state = sign_in(&app, &rx, padded);
    let user = state
        .chrome
        .current_user
        .expect("surrounding whitespace should be tolerated");
    assert_eq!(user.pubkey, keys.public_key().to_hex());
}
