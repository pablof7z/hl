//! Test-only proof that Highlighter can consume the supported new-NMP
//! boundary without starting a second production engine.

use nmp::{nmp_threads_live, Engine, EngineConfig, LiveQuery, RelayUrl};
use nmp_next_nip29::group_discovery_demand;

#[tokio::test]
async fn facade_lifecycle_opens_group_discovery_and_reopens_its_store() {
    let _serial = crate::kernel::new_nmp::NEW_NMP_TEST_LOCK.lock().await;
    let threads_before = nmp_threads_live();
    let store_dir = tempfile::tempdir().expect("temporary NMP store directory");
    let store_path = store_dir.path().join("new-nmp-canary.redb");
    let config = EngineConfig {
        store_path: Some(store_path.to_string_lossy().into_owned()),
        allowed_local_relay_hosts: vec!["127.0.0.1".to_string()],
        ..EngineConfig::default()
    };

    let engine = Engine::new(config.clone()).expect("new NMP engine must start");
    let host = RelayUrl::parse("ws://127.0.0.1:1").expect("fixed relay URL must parse");
    let query = LiveQuery(group_discovery_demand(host));
    let subscription = engine
        .observe(query, None)
        .expect("host-scoped NIP-29 observation must open");

    subscription.cancel();
    engine.shutdown();
    assert!(store_path.exists(), "persistent store must be created");

    let reopened = Engine::new(config).expect("the closed store must reopen");
    reopened.shutdown();

    assert_eq!(
        nmp_threads_live(),
        threads_before,
        "observation cancellation and shutdown must leave no NMP worker alive"
    );
}
