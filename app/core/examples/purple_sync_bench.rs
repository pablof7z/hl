//! Bench the purplepag.es NIP-77 negentropy sync against a plain REQ
//! fetch for the same filter, on real data.
//!
//! For each test pubkey:
//!   1. Fetch their kind:3 to learn the follow set.
//!   2. Open a fresh nostrdb instance, then time REQ-based
//!      `fetch_events({ kinds:[0,3,10002], authors:<follows> })` against
//!      purplepag.es.
//!   3. Open another fresh nostrdb instance, then time
//!      `client.sync_with([purple], same_filter, SyncDirection::Down)`.
//!
//! Reports wall-clock time + event count for each path so we can verify
//! negentropy is actually faster (and not wildly more expensive) before
//! shipping the change to the iOS app.
//!
//! Run with:
//!   cargo run --release --example purple_sync_bench
//!
//! Standalone tool; not on the runtime path.

use std::time::{Duration, Instant};

use nostr_ndb::NdbDatabase;
use nostr_sdk::prelude::*;
use nostrdb::{Config as NdbConfig, Ndb};

/// Relays we want to A/B against. The first one is purplepag.es per the
/// initial premise; the others are known strfry deployments that
/// definitely advertise NIP-77 — useful as a control to rule out a
/// nostr-sdk usage error.
const RELAYS_UNDER_TEST: &[&str] = &[
    "wss://purplepag.es",
    "wss://nostr.wine",
    "wss://relay.damus.io",
];

const TEST_USERS: &[(&str, &str)] = &[
    (
        "_@f7z.io (Pablo)",
        "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52",
    ),
    (
        "fiatjaf",
        "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
    ),
    (
        "vitor (Amethyst)",
        "460c25e682fda7832b52d1f22d3d22b3176d972f60dcdc3212ed8c92ef85065c",
    ),
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Purple sync bench — negentropy vs REQ");
    println!("=====================================\n");
    println!("Cold cache for each path. Each test user gets two fresh");
    println!("nostrdb instances so neither method gets a head start.\n");

    // First, learn each user's follow set via a shared bootstrap client.
    let bootstrap = Client::default();
    for r in RELAYS_UNDER_TEST {
        bootstrap.add_relay(*r).await?;
    }
    bootstrap.add_relay("wss://nos.lol").await?;
    bootstrap.connect().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    for (name, pk_hex) in TEST_USERS {
        let pk = PublicKey::from_hex(pk_hex)?;
        let follows = match learn_follows(&bootstrap, pk).await {
            Ok(f) if !f.is_empty() => f,
            Ok(_) => {
                eprintln!("skip {name}: empty follow list");
                continue;
            }
            Err(e) => {
                eprintln!("skip {name}: {e}");
                continue;
            }
        };

        println!("===== {name} =====");
        println!("  follows: {}", follows.len());

        let filter = || {
            Filter::new()
                .kinds([Kind::Custom(0), Kind::Custom(3), Kind::Custom(10002)])
                .authors(follows.clone())
        };

        for relay in RELAYS_UNDER_TEST {
            println!("  --- {relay} ---");
            let (req_count, req_elapsed) = bench_req(relay, filter()).await?;
            println!(
                "    REQ        → {:>5} events in {:>6} ms",
                req_count,
                req_elapsed.as_millis()
            );

            let (sync_count, sync_elapsed) = bench_sync(relay, filter()).await?;
            println!(
                "    NEGENTROPY → {:>5} events in {:>6} ms{}",
                sync_count,
                sync_elapsed.as_millis(),
                if sync_count == 0 && sync_elapsed.as_secs() >= 9 {
                    "  (timed out — relay likely doesn't support NIP-77)"
                } else {
                    ""
                }
            );
        }
        println!();
    }

    Ok(())
}

async fn learn_follows(client: &Client, pk: PublicKey) -> anyhow::Result<Vec<PublicKey>> {
    let filter = Filter::new().kinds([Kind::ContactList]).author(pk);
    let events = client
        .fetch_events(filter, Duration::from_secs(15))
        .await?;
    let Some(ev) = events.into_iter().max_by_key(|e| e.created_at) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<PublicKey> = Vec::new();
    let mut seen = std::collections::BTreeSet::<PublicKey>::new();
    for tag in ev.tags.iter() {
        let s = tag.as_slice();
        if s.first().map(String::as_str) != Some("p") {
            continue;
        }
        if let Some(hex) = s.get(1) {
            if let Ok(p) = PublicKey::from_hex(hex) {
                if seen.insert(p) {
                    out.push(p);
                }
            }
        }
    }
    Ok(out)
}

/// Open a fresh nostrdb under a unique tmpdir so the bench starts cold.
fn fresh_client() -> anyhow::Result<(Client, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    let cfg = NdbConfig::new().set_mapsize(256 * 1024 * 1024);
    let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg)?;
    let database = NdbDatabase::from(ndb);
    let client = Client::builder().database(database).build();
    Ok((client, tmp))
}

async fn bench_req(relay: &str, filter: Filter) -> anyhow::Result<(usize, Duration)> {
    let (client, _tmp) = fresh_client()?;
    client.add_relay(relay).await?;
    client.connect().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let started = Instant::now();
    let events = client
        .fetch_events(filter, Duration::from_secs(45))
        .await?;
    let elapsed = started.elapsed();
    Ok((events.len(), elapsed))
}

async fn bench_sync(relay: &str, filter: Filter) -> anyhow::Result<(usize, Duration)> {
    let (client, _tmp) = fresh_client()?;
    client.add_relay(relay).await?;
    client.connect().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let opts = SyncOptions::default()
        .direction(SyncDirection::Down)
        .initial_timeout(Duration::from_secs(10));

    let started = Instant::now();
    let output = client.sync_with([relay], filter, &opts).await?;
    let elapsed = started.elapsed();
    Ok((output.val.received.len(), elapsed))
}
