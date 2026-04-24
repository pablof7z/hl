//! Real-data validation for the outbox planner.
//!
//! NIP-65 has *no ordering* on relays in a `kind:10002`: position 1 has
//! the same significance as position 5. So "top N in listed order" is
//! effectively picking N at random — it limits the planner's input for
//! no good reason and shrinks the overlap pool the set-cover can mine.
//!
//! This harness fetches each test user's `kind:3`, then every follow's
//! `kind:10002`, then runs the planner under three per-pubkey-cap
//! regimes side-by-side so we can quantify what cutting the input to
//! "top 2" was actually costing:
//!
//!   - `top_n = 2`   — the original (broken) behaviour
//!   - `top_n = 5`   — moderate sanity bound
//!   - `top_n = all` — feed every write relay; let set-cover decide
//!
//! For each combination we report shards / coverage / avg authors per
//! shard at multiple output caps. The detailed shard breakdown is shown
//! at `top_n = all` because that's the production-correct mode.
//!
//! Run with:
//!   cargo run --release --example outbox_stats
//!
//! Pure validation tool; not on the iOS app's runtime path.

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

const OUTPUT_CAPS: &[usize] = &[6, 8, 10, 15, 25];

/// Per-pubkey input caps to compare. NIP-65 has no ordering, so these
/// are sanity bounds, not priority signals.
#[derive(Clone, Copy)]
struct InputCap {
    label: &'static str,
    cap: usize,
}
const INPUT_CAPS: &[InputCap] = &[
    InputCap { label: "top_n=2  ", cap: 2 },
    InputCap { label: "top_n=5  ", cap: 5 },
    InputCap { label: "top_n=all", cap: usize::MAX },
];

/// Detail mode: production-correct (all write relays).
const DETAIL_INPUT_CAP: usize = usize::MAX;
const DETAIL_OUTPUT_CAP: usize = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Outbox Planner — Real-Data Stats");
    println!("================================\n");
    println!("NIP-65 has no relay ordering: position 1 = position 5.");
    println!("Comparing top_n ∈ {{2, 5, all}} to quantify the cost of");
    println!("artificially capping the planner's input.\n");

    let client = Client::default();
    client.add_relay(PURPLE).await?;
    for r in FALLBACK {
        client.add_relay(*r).await?;
    }
    client.connect().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Connected to {PURPLE} (primary) + {} fallback relays.\n", FALLBACK.len());

    for (name, pk_hex) in TEST_USERS {
        let pk = match PublicKey::from_hex(pk_hex) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip {name}: {e}");
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

    // 1. kind:3
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

    // 2. kind:10002 for all follows
    let mut nip65_by_author: HashMap<PublicKey, Event> = HashMap::new();
    for chunk in follows.chunks(500) {
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
    let with_nip65 = follows.iter().filter(|pk| nip65_by_author.contains_key(pk)).count();
    println!(
        "  follows with kind:10002 cached: {} ({:.1}%)",
        with_nip65,
        100.0 * with_nip65 as f64 / follows.len() as f64
    );

    // Distribution of how many write relays per author (to size the cap)
    let relay_counts: Vec<usize> = follows
        .iter()
        .filter_map(|pk| nip65_by_author.get(pk))
        .map(|ev| outbox::write_relays_from_event(ev, usize::MAX).len())
        .collect();
    if !relay_counts.is_empty() {
        let mut sorted = relay_counts.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        let p90 = sorted[(sorted.len() * 9) / 10];
        let max = *sorted.last().unwrap();
        let avg = sorted.iter().sum::<usize>() as f64 / sorted.len() as f64;
        println!(
            "  write relays per author — avg={:.1}  median={}  p90={}  max={}",
            avg, median, p90, max
        );
    }

    // 3. Build per-pubkey maps for each input cap
    let mut per_pubkey_by_cap: HashMap<usize, HashMap<PublicKey, Vec<String>>> = HashMap::new();
    for ic in INPUT_CAPS {
        let mut map: HashMap<PublicKey, Vec<String>> = HashMap::new();
        for f in &follows {
            if let Some(ev) = nip65_by_author.get(f) {
                map.insert(*f, outbox::write_relays_from_event(ev, ic.cap));
            } else {
                map.insert(*f, Vec::new());
            }
        }
        per_pubkey_by_cap.insert(ic.cap, map);
    }

    // 4. Side-by-side comparison table
    println!();
    println!(
        "  output_cap | {} | {} | {}",
        INPUT_CAPS[0].label, INPUT_CAPS[1].label, INPUT_CAPS[2].label
    );
    println!(
        "             | relays  covered     | relays  covered     | relays  covered"
    );
    println!(
        "  -----------+----------------------+----------------------+--------------------"
    );
    for &out_cap in OUTPUT_CAPS {
        print!("  {:>10} |", out_cap);
        for ic in INPUT_CAPS {
            let plan =
                outbox::compute_outbox_plan(per_pubkey_by_cap[&ic.cap].clone(), out_cap);
            let covered = follows.len() - plan.uncovered.len();
            print!(
                " {:>2}     {:>4} ({:>5.1}%) |",
                plan.shards.len(),
                covered,
                100.0 * covered as f64 / follows.len() as f64
            );
        }
        println!();
    }

    // 5. Detailed plan at production-correct settings
    println!();
    println!(
        "  Plan @ output_cap={DETAIL_OUTPUT_CAP}, input_cap=ALL (production-correct):"
    );
    let plan = outbox::compute_outbox_plan(
        per_pubkey_by_cap[&DETAIL_INPUT_CAP].clone(),
        DETAIL_OUTPUT_CAP,
    );
    for (i, shard) in plan.shards.iter().enumerate() {
        println!(
            "    {:>2}. {:<46} → {:>4} authors",
            i + 1,
            truncate(&shard.url, 46),
            shard.authors.len()
        );
    }
    if !plan.uncovered.is_empty() {
        let no_nip65 = plan
            .uncovered
            .iter()
            .filter(|p| {
                per_pubkey_by_cap[&DETAIL_INPUT_CAP]
                    .get(p)
                    .map(|v| v.is_empty())
                    .unwrap_or(true)
            })
            .count();
        println!(
            "    fallback (user's read relays): {} uncovered  ({} have no kind:10002, {} trimmed by cap)",
            plan.uncovered.len(),
            no_nip65,
            plan.uncovered.len() - no_nip65
        );
    }

    // 6. Sanity check
    let mut all_ok = true;
    for shard in &plan.shards {
        for a in &shard.authors {
            let listed_it = per_pubkey_by_cap[&DETAIL_INPUT_CAP]
                .get(a)
                .map(|v| v.contains(&shard.url))
                .unwrap_or(false);
            if !listed_it {
                all_ok = false;
                eprintln!(
                    "  !! BUG: author {} in shard {} did not list that relay",
                    a.to_hex(),
                    shard.url
                );
            }
        }
    }
    if all_ok {
        println!(
            "  per-relay filtering OK: each shard contains only authors that listed its relay."
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
