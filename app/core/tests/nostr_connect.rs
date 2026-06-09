//! NIP-46 surface smoke tests. These exercise the URI construction and
//! input validation paths — neither test actually waits for a live remote
//! signer to respond.

use highlighter_core::{HighlighterCore, LoginInputAction};
use std::sync::Arc;
use tempfile::TempDir;

fn isolated_core() -> (Arc<HighlighterCore>, TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
    (core, tmp)
}

#[tokio::test]
async fn start_default_nostr_connect_returns_valid_uri_with_callback() {
    let (core, _tmp) = isolated_core();

    let snapshot = core
        .start_default_nostr_connect("highlighter://nip46".into())
        .await;
    assert!(
        snapshot.started,
        "start_default_nostr_connect: {}",
        snapshot.error_message
    );
    let uri = snapshot.uri;

    // Shape: nostrconnect://<64-hex pubkey>?<query>
    assert!(uri.starts_with("nostrconnect://"), "got: {uri}");

    // Relay must be Primal's bunker relay (hardcoded per spec).
    assert!(
        uri.contains("relay=wss://relay.primal.net"),
        "missing primal relay in URI: {uri}"
    );

    // App-owned permission policy must come from Rust defaults.
    assert!(
        uri.contains("perms=sign_event:11"),
        "missing sign_event:11 perm: {uri}"
    );

    // App name must be URL-encoded in the query string.
    assert!(
        uri.contains("name=Highlighter"),
        "missing name param: {uri}"
    );

    // Secret param for the connect handshake.
    assert!(uri.contains("secret="), "missing secret param: {uri}");

    // Platform callback is supplied by native, but Rust owns query assembly.
    assert!(
        uri.contains("callback=highlighter%3A%2F%2Fnip46"),
        "missing encoded callback param: {uri}"
    );
}

#[test]
fn classify_login_input_matches_manual_login_policy() {
    let (core, _tmp) = isolated_core();
    assert_eq!(
        core.classify_login_input(" nostr:nsec1example ".into()),
        LoginInputAction::Nsec {
            nsec: "nsec1example".into()
        }
    );
    assert_eq!(
        core.classify_login_input("nostrconnect://example".into()),
        LoginInputAction::Bunker {
            uri: "nostrconnect://example".into()
        }
    );
    assert_eq!(
        core.classify_login_input(" nostr: ".into()),
        LoginInputAction::Empty
    );
    assert_eq!(
        core.classify_login_input("npub1example".into()),
        LoginInputAction::Invalid {
            message: "Enter an nsec1… or bunker:// URI.".into()
        }
    );
}

#[tokio::test]
async fn pair_bunker_rejects_garbage() {
    let (core, _tmp) = isolated_core();

    // Leading `nostr:` is the only thing `normalize_bunker_uri` strips —
    // everything else must parse as a valid NIP-46 URI.
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
        let res = core.pair_bunker(case.to_string()).await;
        assert!(
            !res.error_message.is_empty(),
            "pair_bunker should reject {case:?} but got {:?}",
            res
        );
    }
}
