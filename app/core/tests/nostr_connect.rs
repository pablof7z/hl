//! NIP-46 surface smoke tests. These exercise the URI construction and
//! input validation paths — neither test actually waits for a live remote
//! signer to respond.

use highlighter_core::{
    HighlighterAppAction, HighlighterAppConfig, HighlighterAppReconciler, HighlighterAppState,
    HighlighterNmpApp, HighlighterSessionCredential, HighlighterToastKind,
};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[derive(Debug, Clone)]
enum TestUpdate {
    State(HighlighterAppState),
    PersistSessionCredential,
    ClearSessionCredentials,
    OpenExternalUrl(String),
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

    fn on_open_external_url(&self, url: String) {
        let _ = self.tx.send(TestUpdate::OpenExternalUrl(url));
    }
}

fn isolated_app() -> (Arc<HighlighterNmpApp>, Receiver<TestUpdate>, TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = HighlighterNmpApp::new(HighlighterAppConfig {
        data_dir: Some(tmp.path().join("ndb").to_string_lossy().into_owned()),
        visible_limit: 8,
        emit_hz: 30,
    });
    let (tx, rx) = channel();
    app.listen_for_updates(Arc::new(TestReconciler { tx }));
    let _ = next_state(&rx);
    (app, rx, tmp)
}

fn next_update(rx: &Receiver<TestUpdate>) -> TestUpdate {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("app update within timeout")
}

fn next_state(rx: &Receiver<TestUpdate>) -> HighlighterAppState {
    for _ in 0..16 {
        if let TestUpdate::State(state) = next_update(rx) {
            return state;
        }
    }
    panic!("state update within timeout")
}

#[test]
fn start_nostr_connect_emits_valid_external_uri() {
    let (app, rx, _tmp) = isolated_app();

    app.dispatch(HighlighterAppAction::StartNostrConnect {
        callback_url: "highlighter://nip46".into(),
    });

    let mut uri = None;
    for _ in 0..16 {
        match next_update(&rx) {
            TestUpdate::OpenExternalUrl(url) => {
                uri = Some(url);
                break;
            }
            TestUpdate::State(_)
            | TestUpdate::PersistSessionCredential
            | TestUpdate::ClearSessionCredentials => {}
        }
    }
    let uri = uri.expect("start_nostr_connect should request opening a URI");

    let parsed = url::Url::parse(&uri).expect("nostrconnect URI should parse");
    assert_eq!(parsed.scheme(), "nostrconnect", "got: {uri}");
    let pubkey = parsed.host_str().expect("missing nostrconnect pubkey host");
    assert_eq!(pubkey.len(), 64, "got: {uri}");
    assert!(pubkey.chars().all(|c| c.is_ascii_hexdigit()), "got: {uri}");
    let query: HashMap<String, String> = parsed.query_pairs().into_owned().collect();

    assert!(
        query
            .get("relay")
            .is_some_and(|v| v == "wss://relay.primal.net"),
        "missing relay in URI: {uri}"
    );

    // Perms must round-trip. We passed a specific subset — check at least one
    // entry made it through.
    assert!(
        query
            .get("perms")
            .is_some_and(|v| v.contains("sign_event:11")),
        "missing sign_event:11 perm: {uri}"
    );

    assert!(
        query.get("name").is_some_and(|v| v == "Highlighter"),
        "missing name param: {uri}"
    );

    assert!(
        query.get("secret").is_some_and(|v| !v.is_empty()),
        "missing secret param: {uri}"
    );
}

#[test]
fn pair_bunker_rejects_garbage() {
    let cases = [
        "",
        "   ",
        "not a uri",
        // nsec1 is a different URI format — must be rejected here.
        "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
        // Valid scheme but missing required host/params.
        "bunker://",
        "bunker://notapubkey",
        // Valid URI shape but points at a bad relay URL.
        "bunker://79dff8f82963424e0bb02708a22e44b4980893e3a4be0fa3cb60a43b946764e3?relay=::not-a-url",
    ];

    for case in cases {
        let (app, rx, _tmp) = isolated_app();
        app.dispatch(HighlighterAppAction::PairBunker {
            uri: case.to_string(),
            persist: false,
            clear_stored_on_failure: false,
        });
        let mut saw_error = false;
        for _ in 0..8 {
            let state = next_state(&rx);
            saw_error = state
                .toast
                .as_ref()
                .is_some_and(|toast| toast.kind == HighlighterToastKind::Error);
            if saw_error {
                break;
            }
        }
        assert!(saw_error, "pair_bunker should reject {case:?}");
    }
}
