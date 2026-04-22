//! NIP-46 surface smoke tests. These exercise the URI construction and
//! input validation paths — neither test actually waits for a live remote
//! signer to respond.

use highlighter_core::{HighlighterCore, NostrConnectOptions};
use std::sync::Arc;
use tempfile::TempDir;

fn isolated_core() -> (Arc<HighlighterCore>, TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
    (core, tmp)
}

#[tokio::test]
async fn start_nostr_connect_returns_valid_uri() {
    let (core, _tmp) = isolated_core();

    let options = NostrConnectOptions {
        name: "Highlighter".into(),
        url: "https://highlighter.com".into(),
        image: "https://highlighter.com/icon.png".into(),
        perms: "sign_event:11,sign_event:9802,nip44_encrypt".into(),
    };

    let uri = core
        .start_nostr_connect(options)
        .await
        .expect("start_nostr_connect should return a URI");

    // Shape: nostrconnect://<64-hex pubkey>?<query>
    assert!(uri.starts_with("nostrconnect://"), "got: {uri}");

    // Relay must be Primal's bunker relay (hardcoded per spec).
    assert!(
        uri.contains("relay=wss://relay.primal.net"),
        "missing primal relay in URI: {uri}"
    );

    // Perms must round-trip. We passed a specific subset — check at least one
    // entry made it through.
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
            res.is_err(),
            "pair_bunker should reject {case:?} but got {:?}",
            res
        );
    }
}
