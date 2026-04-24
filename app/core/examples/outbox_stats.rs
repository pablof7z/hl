//! Real-data validation for the outbox planner.
//!
//! For each test pubkey:
//!   1. Fetch their kind:3 contact list from purplepag.es (+ a couple of
//!      generic relays as fallback so we don't miss data when purple is
//!      cold).
//!   2. Fetch all kind:10002 events for the people they follow.
//!   3. Build the per-pubkey "top 2 write relays" map the planner expects.
//!   4. Run `compute_outbox_plan` at several caps and report:
//!        - relays selected
//!        - authors covered (% of follows)
//!        - average authors per shard
//!        - per-shard breakdown at cap = OUTBOX_MAX_RELAYS (10)
//!
//! Run with:
//!   cargo run --release --example outbox_stats
//!
//! The output exists to validate the algorithm against real Nostr data;
//! it is not part of the iOS app's runtime path.

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use highlighter_core::outbox;
use nostr_sdk::prelude::*;

const PURPLE: &str = "wss://purplepag.es";
const FALLBACK: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://relay.primal.net",
];

/// Hand-picked test users covering different network sizes and centres of
/// gravity. Each entry is (display name, hex pubkey).
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
        "jb55",
        "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245",
    ),
    (
        "jack",
        "82341f882b6eabcd2ba7f1ef90aad961cf074af15b9ef44a09f9d2a8fbfbe6a2",
    ),
    (
        "hodlbod",
        "97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad00a0d3",
    ),
    (
        "vitor (Amethyst)",
        "460c25e682fda7832b52d1f22d3d22b3176d972f60dcdc3212ed8c92ef85065c",
    ),
];

const CAPS_TO_REPORT: &[usize] = &[4, 6, 8, 10, 15, 25, 50];
const PLAN_DETAIL_CAP: usize = 10;
const TOP_N_PER_PUBKEY: usize = 2;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Outbox Planner — Real-Data Stats");
    println!("================================\n");

    let client = Client::default();
    client.add_relay(PURPLE).await?;
    for r in FALLBACK {
        client.add_relay(*r).await?;
    }
    client.connect().await;
    // Give relays a beat to actually open the sockets.
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Connected to {PURPLE} (primary) and {} fallback relays.\n", FALLBACK.len());
    println!("Filter routing rule under test: each shard sends ONLY the");
    println!("authors that listed that relay in their kind:10002 — not");
    println!("the full follow set.\n");

    for (name, pk_hex) in TEST_USERS {
        let pk = match PublicKey::from_hex(pk_hex) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip {name}: bad pubkey: {e}");
                continue;
            }
        };
        if let Err(e) = analyze(&client, name, pk).await {
            eprintln!("analyze {name}: {e}");
        }
    }

    Ok(())
}

async fn analyze(client: &Client, name: &str, pk: PublicKey) -> anyhow::Result<()> {
    println!("===== {name} =====");
    println!("pubkey: {}", pk.to_hex());

    // 1. kind:3 — their contact list.
    let contacts_filter = Filter::new().kinds([Kind::ContactList]).author(pk);
    let events = client
        .fetch_events(contacts_filter, Duration::from_secs(15))
        .await?;
    let Some(contacts_event) = events.into_iter().max_by_key(|e| e.created_at) else {
        println!("  (no kind:3 found within timeout)\n");
        return Ok(());
    };

    let follows: Vec<PublicKey> = {
        let mut seen: BTreeSet<PublicKey> = BTreeSet::new();
        for tag in contacts_event.tags.iter() {
            let s = tag.as_slice();
            if s.first().map(String::as_str) != Some("p") {
                continue;
            }
            if let Some(hex) = s.get(1) {
                if let Ok(p) = PublicKey::from_hex(hex) {
                    seen.insert(p);
                }
            }
        }
        seen.into_iter().collect()
    };

    println!("  follows (kind:3 p-tags, deduped): {}", follows.len());

    if follows.is_empty() {
        println!();
        return Ok(());
    }

    // 2. kind:10002 for all follows. Chunk to avoid blowing past relay
    //    filter-size limits (purplepages caps around 1000 authors).
    let mut nip65_by_author: HashMap<PublicKey, Event> = HashMap::new();
    let chunk_size = 500usize;
    for chunk in follows.chunks(chunk_size) {
        let filter = Filter::new()
            .kinds([Kind::RelayList])
            .authors(chunk.to_vec());
        let events = client.fetch_events(filter, Duration::from_secs(20)).await?;
        for ev in events {
            match nip65_by_author.get(&ev.pubkey) {
                Some(prev) if prev.created_at >= ev.created_at => {}
                _ => {
                    nip65_by_author.insert(ev.pubkey, ev);
                }
            }
        }
    }
    let with_nip65 = follows
        .iter()
        .filter(|pk| nip65_by_author.contains_key(pk))
        .count();
    let coverage_pct = 100.0 * with_nip65 as f64 / follows.len() as f64;
    println!(
        "  follows with kind:10002 cached: {} ({:.1}%)",
        with_nip65, coverage_pct
    );

    // 3. Build per-pubkey "top 2 write relays" map.
    let mut per_pubkey: HashMap<PublicKey, Vec<String>> = HashMap::new();
    for f in &follows {
        if let Some(ev) = nip65_by_author.get(f) {
            per_pubkey.insert(*f, outbox::write_relays_from_event(ev, TOP_N_PER_PUBKEY));
        } else {
            per_pubkey.insert(*f, Vec::new());
        }
    }

    let unique_relays: BTreeSet<&String> = per_pubkey.values().flat_map(|v| v.iter()).collect();
    println!(
        "  unique write relays across follows: {}",
        unique_relays.len()
    );

    // 4. Cap-vs-coverage table.
    println!();
    println!("  cap | relays | covered (% of follows) | avg authors/shard");
    println!("  ----+--------+------------------------+------------------");
    for &cap in CAPS_TO_REPORT {
        let plan = outbox::compute_outbox_plan(per_pubkey.clone(), cap);
        let covered = follows.len() - plan.uncovered.len();
        let avg = if plan.shards.is_empty() {
            0.0
        } else {
            plan.shards.iter().map(|s| s.authors.len()).sum::<usize>() as f64
                / plan.shards.len() as f64
        };
        println!(
            "  {:>3} | {:>6} | {:>5} ({:>5.1}%)        | {:>6.1}",
            cap,
            plan.shards.len(),
            covered,
            100.0 * covered as f64 / follows.len() as f64,
            avg
        );
    }

    // 5. Detailed plan at the production cap.
    println!();
    println!("  Plan at cap = {PLAN_DETAIL_CAP} (relay → distinct authors queried):");
    let plan = outbox::compute_outbox_plan(per_pubkey.clone(), PLAN_DETAIL_CAP);
    for (i, shard) in plan.shards.iter().enumerate() {
        println!(
            "    {:>2}. {:<46} → {:>4} authors",
            i + 1,
            truncate(&shard.url, 46),
            shard.authors.len()
        );
    }
    if !plan.uncovered.is_empty() {
        println!(
            "    fallback (user's read relays): {} authors uncovered ({} have no kind:10002 cached, the rest got trimmed by the cap)",
            plan.uncovered.len(),
            plan.uncovered
                .iter()
                .filter(|p| per_pubkey.get(p).map(|v| v.is_empty()).unwrap_or(true))
                .count()
        );
    }

    // 6. Sanity check — every shard's `authors` is a subset of the
    //    population that listed that shard's relay. Per the user's spec:
    //    "we shouldnt sent all authors to that relay; we only send the
    //    users who have that relay in their 10002".
    let mut all_ok = true;
    for shard in &plan.shards {
        for a in &shard.authors {
            let listed_it = per_pubkey
                .get(a)
                .map(|v| v.contains(&shard.url))
                .unwrap_or(false);
            if !listed_it {
                all_ok = false;
                eprintln!(
                    "  !! BUG: author {} appears in shard {} but did not list that relay",
                    a.to_hex(),
                    shard.url
                );
            }
        }
    }
    if all_ok {
        println!(
            "  per-relay author filtering verified: every shard contains only authors that listed its relay."
        );
    }

    println!();
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n.saturating_sub(1)])
    }
}
