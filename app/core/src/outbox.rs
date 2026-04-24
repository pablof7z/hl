//! NIP-65 outbox routing.
//!
//! Two pieces:
//!
//! 1. [`write_relays_for_pubkey`] — given a pubkey, read their newest cached
//!    `kind:10002` and return up to `top_n` relays they write to (in their
//!    own listed order). The caller decides `top_n`; we use 2 by default to
//!    bound the worst-case fan-out.
//!
//! 2. [`compute_outbox_plan`] — pure greedy set-cover. Input: a per-pubkey
//!    map of "preferred write relays". Output: a list of (relay, authors)
//!    shards plus an `uncovered` list for authors whose relays we couldn't
//!    afford to keep within `max_total_relays`.
//!
//! The set-cover is the classic O(n log n) greedy: at each step, pick the
//! relay that covers the largest set of still-uncovered pubkeys; tie-break
//! by relay URL for determinism. With the per-pubkey cap, the input is
//! small enough that the naïve loop is fast in practice (a 500-follow user
//! with cap=2 yields ≤ 1000 (relay,pubkey) edges).
//!
//! The algorithm naturally exploits intersections: if 80% of follows
//! publish on `relay.damus.io`, that relay is picked first and covers
//! everyone in one connection. Niche follows force additional picks; the
//! hard cap stops the long tail. Authors past the cap fall back to the
//! user's own read relays at the call site.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;

const KIND_RELAY_LIST: u16 = 10002;

/// Plan output: which relays to subscribe to and which authors each shard
/// covers, plus the leftover authors that didn't fit under the cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxPlan {
    pub shards: Vec<RelayShard>,
    pub uncovered: Vec<PublicKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayShard {
    pub url: String,
    pub authors: Vec<PublicKey>,
}

/// Newest cached kind:10002 for `pubkey_hex`, parsed into write-marked
/// relays in their listed order. Returns up to `top_n` URLs. NIP-65
/// markers: `["r", url, "write"]` ⇒ write only, `["r", url]` (no marker)
/// ⇒ both — both count for outbox. Read-only entries are excluded.
pub fn write_relays_for_pubkey(
    ndb: &Ndb,
    pubkey_hex: &str,
    top_n: usize,
) -> Result<Vec<String>, CoreError> {
    if pubkey_hex.is_empty() || top_n == 0 {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_RELAY_LIST as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query relay list: {e}")))?;

    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }
    let Some(event) = newest else {
        return Ok(Vec::new());
    };
    Ok(write_relays_from_event(&event, top_n))
}

/// Pure parse: extract write-marked relays from a kind:10002, in their
/// listed order, deduped, capped at `top_n`. Public for tests.
pub fn write_relays_from_event(event: &Event, top_n: usize) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<String> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("r") {
            continue;
        }
        let Some(url) = slice.get(1) else { continue };
        let url = normalize_relay_url(url);
        if url.is_empty() {
            continue;
        }
        let is_write = match slice.get(2).map(String::as_str) {
            Some("read") => false,
            Some("write") => true,
            _ => true,
        };
        if !is_write {
            continue;
        }
        if seen.insert(url.clone()) {
            out.push(url);
            if out.len() == top_n {
                break;
            }
        }
    }
    out
}

/// Greedy set-cover over `(relay → authors)`. Picks at most `max_total_relays`
/// relays, each tie-broken by URL string for deterministic output. Authors
/// without any preferred relay land in `uncovered`. Authors whose all
/// preferred relays got cut by the cap also land in `uncovered`.
pub fn compute_outbox_plan(
    per_pubkey: HashMap<PublicKey, Vec<String>>,
    max_total_relays: usize,
) -> OutboxPlan {
    if per_pubkey.is_empty() || max_total_relays == 0 {
        return OutboxPlan {
            shards: Vec::new(),
            uncovered: per_pubkey.into_keys().collect(),
        };
    }

    // Authors with at least one candidate relay.
    let mut uncovered: BTreeSet<PublicKey> = per_pubkey
        .iter()
        .filter(|(_, urls)| !urls.is_empty())
        .map(|(pk, _)| *pk)
        .collect();
    // Authors with zero candidates — straight to uncovered output.
    let mut already_uncovered: Vec<PublicKey> = per_pubkey
        .iter()
        .filter(|(_, urls)| urls.is_empty())
        .map(|(pk, _)| *pk)
        .collect();

    // Inverse index: relay → set of authors that listed it.
    let mut relay_to_authors: BTreeMap<String, BTreeSet<PublicKey>> = BTreeMap::new();
    for (pk, urls) in &per_pubkey {
        for url in urls {
            relay_to_authors.entry(url.clone()).or_default().insert(*pk);
        }
    }

    let mut chosen: Vec<RelayShard> = Vec::new();

    while !uncovered.is_empty() && chosen.len() < max_total_relays {
        // Find the relay covering the most still-uncovered authors. Stable
        // tie-break: prefer the URL with the lower lexicographic order so
        // identical inputs yield identical plans.
        let mut best: Option<(usize, String)> = None;
        for (url, authors) in &relay_to_authors {
            let count = authors.intersection(&uncovered).count();
            if count == 0 {
                continue;
            }
            match &best {
                None => best = Some((count, url.clone())),
                Some((bc, bu)) if count > *bc || (count == *bc && url < bu) => {
                    best = Some((count, url.clone()));
                }
                _ => {}
            }
        }
        let Some((_, picked_url)) = best else { break };
        let covered: Vec<PublicKey> = relay_to_authors
            .get(&picked_url)
            .map(|set| set.intersection(&uncovered).copied().collect())
            .unwrap_or_default();
        for pk in &covered {
            uncovered.remove(pk);
        }
        chosen.push(RelayShard {
            url: picked_url.clone(),
            authors: covered,
        });
        // Don't pick the same relay twice.
        relay_to_authors.remove(&picked_url);
    }

    let mut leftover: Vec<PublicKey> = uncovered.into_iter().collect();
    leftover.append(&mut already_uncovered);

    OutboxPlan {
        shards: chosen,
        uncovered: leftover,
    }
}

/// Light normalization so `wss://relay.example.com/` and
/// `wss://relay.example.com` collapse to the same shard. Lowercases scheme
/// + host, strips a trailing slash. Path/query are preserved verbatim.
fn normalize_relay_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let trimmed = trimmed.trim_end_matches('/');
    if let Some(idx) = trimmed.find("://") {
        let (scheme, rest) = trimmed.split_at(idx);
        let rest = &rest[3..];
        let (host, tail) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        format!(
            "{}://{}{}",
            scheme.to_ascii_lowercase(),
            host.to_ascii_lowercase(),
            tail
        )
    } else {
        trimmed.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(seed: u8) -> PublicKey {
        let bytes = [seed; 32];
        PublicKey::from_slice(&bytes).expect("valid pk")
    }

    fn sign_relay_list(keys: &Keys, entries: Vec<(&str, Option<&str>)>) -> Event {
        let tags: Vec<Tag> = entries
            .into_iter()
            .map(|(url, marker)| {
                let mut parts = vec!["r".to_string(), url.to_string()];
                if let Some(m) = marker {
                    parts.push(m.to_string());
                }
                Tag::parse(parts).expect("tag")
            })
            .collect();
        EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn write_relays_extracts_in_order_caps_at_top_n() {
        let me = Keys::generate();
        let event = sign_relay_list(
            &me,
            vec![
                ("wss://r1", Some("write")),
                ("wss://r2", None),
                ("wss://r3", Some("read")),
                ("wss://r4", Some("write")),
            ],
        );
        let out = write_relays_from_event(&event, 2);
        assert_eq!(out, vec!["wss://r1".to_string(), "wss://r2".to_string()]);
    }

    #[test]
    fn write_relays_skips_read_only() {
        let me = Keys::generate();
        let event = sign_relay_list(
            &me,
            vec![
                ("wss://r1", Some("read")),
                ("wss://r2", Some("write")),
            ],
        );
        let out = write_relays_from_event(&event, 5);
        assert_eq!(out, vec!["wss://r2".to_string()]);
    }

    #[test]
    fn normalize_collapses_trailing_slash_and_case() {
        assert_eq!(normalize_relay_url("WSS://Relay.EXAMPLE.com/"), "wss://relay.example.com");
        assert_eq!(normalize_relay_url("wss://relay.example.com"), "wss://relay.example.com");
    }

    #[test]
    fn outbox_plan_is_empty_for_empty_input() {
        let plan = compute_outbox_plan(HashMap::new(), 5);
        assert!(plan.shards.is_empty());
        assert!(plan.uncovered.is_empty());
    }

    #[test]
    fn outbox_plan_picks_single_relay_when_everyone_overlaps() {
        // a, b, c all write to wss://hub. Greedy picks hub, covers all 3.
        let mut per: HashMap<PublicKey, Vec<String>> = HashMap::new();
        per.insert(pk(1), vec!["wss://hub".into(), "wss://a-only".into()]);
        per.insert(pk(2), vec!["wss://hub".into(), "wss://b-only".into()]);
        per.insert(pk(3), vec!["wss://hub".into(), "wss://c-only".into()]);
        let plan = compute_outbox_plan(per, 5);
        assert_eq!(plan.shards.len(), 1);
        assert_eq!(plan.shards[0].url, "wss://hub");
        assert_eq!(plan.shards[0].authors.len(), 3);
        assert!(plan.uncovered.is_empty());
    }

    #[test]
    fn outbox_plan_uses_greedy_to_minimise_relays() {
        // a → [r1, r2]; b → [r2, r3]; c → [r2, r3]; d → [r3, r4]
        // Greedy: r2 covers {a, b, c}, then r3 covers {d}. Total 2.
        let mut per: HashMap<PublicKey, Vec<String>> = HashMap::new();
        per.insert(pk(1), vec!["wss://r1".into(), "wss://r2".into()]);
        per.insert(pk(2), vec!["wss://r2".into(), "wss://r3".into()]);
        per.insert(pk(3), vec!["wss://r2".into(), "wss://r3".into()]);
        per.insert(pk(4), vec!["wss://r3".into(), "wss://r4".into()]);
        let plan = compute_outbox_plan(per, 5);
        let urls: Vec<&str> = plan.shards.iter().map(|s| s.url.as_str()).collect();
        assert_eq!(urls, vec!["wss://r2", "wss://r3"]);
        assert!(plan.uncovered.is_empty());
    }

    #[test]
    fn outbox_plan_respects_total_relay_cap() {
        // 6 authors, each on a unique relay. Cap=3 → 3 relays chosen, 3 uncovered.
        let mut per: HashMap<PublicKey, Vec<String>> = HashMap::new();
        for i in 0..6u8 {
            per.insert(pk(i + 1), vec![format!("wss://r{i}")]);
        }
        let plan = compute_outbox_plan(per, 3);
        assert_eq!(plan.shards.len(), 3);
        assert_eq!(plan.uncovered.len(), 3);
    }

    #[test]
    fn outbox_plan_marks_authors_with_no_relays_uncovered() {
        let mut per: HashMap<PublicKey, Vec<String>> = HashMap::new();
        per.insert(pk(1), vec!["wss://r1".into()]);
        per.insert(pk(2), vec![]); // no kind:10002 cached
        let plan = compute_outbox_plan(per, 5);
        assert_eq!(plan.shards.len(), 1);
        assert_eq!(plan.uncovered, vec![pk(2)]);
    }

    #[test]
    fn outbox_plan_is_deterministic_with_ties() {
        // a → [r1, r2]; b → [r3, r4]. Each candidate covers one author.
        // Tie-break by URL; r1 < r3, so r1 is picked first.
        let mut per: HashMap<PublicKey, Vec<String>> = HashMap::new();
        per.insert(pk(1), vec!["wss://r1".into(), "wss://r2".into()]);
        per.insert(pk(2), vec!["wss://r3".into(), "wss://r4".into()]);
        let plan = compute_outbox_plan(per.clone(), 5);
        let plan2 = compute_outbox_plan(per, 5);
        assert_eq!(plan, plan2);
        assert_eq!(plan.shards[0].url, "wss://r1");
        assert_eq!(plan.shards[1].url, "wss://r3");
    }
}
