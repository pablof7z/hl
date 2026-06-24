//! Rust-owned onboarding completion state.
//!
//! Native shells choose the visual route, but the durable "has completed
//! onboarding" flag lives in the shared core so iOS and Android agree.

use std::collections::HashSet;

use crate::models::{
    OnboardingInterest, OnboardingInterestChip, OnboardingInterestProjection,
    OnboardingInterestSelection,
};

pub use crate::kernel::onboarding::OnboardingStore;

const MINIMUM_INTERESTS: u32 = 3;
const JACK_PUBKEY: &str = "82341f882b6eabcd2ba7f1ef90aad961cf074af15b9ef44a09f9d2a8fbfbe6a2";
const FIATJAF_PUBKEY: &str = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";

struct InterestSeed {
    id: &'static str,
    emoji: &'static str,
    label: &'static str,
    pubkeys: &'static [&'static str],
}

const INTERESTS: &[InterestSeed] = &[
    InterestSeed {
        id: "philosophy",
        emoji: "🧠",
        label: "Philosophy",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "science_fiction",
        emoji: "🚀",
        label: "Science Fiction",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "technology",
        emoji: "💻",
        label: "Technology",
        pubkeys: &[FIATJAF_PUBKEY, JACK_PUBKEY],
    },
    InterestSeed {
        id: "history",
        emoji: "📜",
        label: "History",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "economics",
        emoji: "📈",
        label: "Economics",
        pubkeys: &[FIATJAF_PUBKEY],
    },
    InterestSeed {
        id: "psychology",
        emoji: "🔬",
        label: "Psychology",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "literature",
        emoji: "📚",
        label: "Literature",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "politics",
        emoji: "🗳️",
        label: "Politics",
        pubkeys: &[],
    },
    InterestSeed {
        id: "bitcoin",
        emoji: "₿",
        label: "Bitcoin",
        pubkeys: &[JACK_PUBKEY, FIATJAF_PUBKEY],
    },
    InterestSeed {
        id: "self_improvement",
        emoji: "🌱",
        label: "Self-improvement",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "science",
        emoji: "🔭",
        label: "Science",
        pubkeys: &[],
    },
    InterestSeed {
        id: "art",
        emoji: "🎨",
        label: "Art",
        pubkeys: &[],
    },
    InterestSeed {
        id: "music",
        emoji: "🎵",
        label: "Music",
        pubkeys: &[],
    },
    InterestSeed {
        id: "design",
        emoji: "✏️",
        label: "Design",
        pubkeys: &[],
    },
    InterestSeed {
        id: "writing",
        emoji: "✍️",
        label: "Writing",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "startups",
        emoji: "⚡️",
        label: "Startups",
        pubkeys: &[JACK_PUBKEY],
    },
    InterestSeed {
        id: "nostr",
        emoji: "🟣",
        label: "Nostr",
        pubkeys: &[FIATJAF_PUBKEY],
    },
    InterestSeed {
        id: "food",
        emoji: "🍳",
        label: "Food",
        pubkeys: &[],
    },
    InterestSeed {
        id: "travel",
        emoji: "🗺️",
        label: "Travel",
        pubkeys: &[],
    },
    InterestSeed {
        id: "health",
        emoji: "🏃",
        label: "Health",
        pubkeys: &[],
    },
];

pub fn interest_catalog() -> Vec<OnboardingInterest> {
    INTERESTS
        .iter()
        .map(|interest| OnboardingInterest {
            id: interest.id.into(),
            emoji: interest.emoji.into(),
            label: interest.label.into(),
        })
        .collect()
}

pub fn interest_selection(selected_ids: Vec<String>) -> OnboardingInterestSelection {
    let selected = selected_interest_set(&selected_ids);
    interest_selection_for_set(&selected)
}

pub fn interest_projection(selected_ids: Vec<String>) -> OnboardingInterestProjection {
    let selected = selected_interest_set(&selected_ids);
    OnboardingInterestProjection {
        interests: INTERESTS
            .iter()
            .map(|interest| OnboardingInterestChip {
                id: interest.id.into(),
                emoji: interest.emoji.into(),
                label: interest.label.into(),
                is_selected: selected.contains(interest.id),
            })
            .collect(),
        selection: interest_selection_for_set(&selected),
    }
}

pub fn toggle_interest_selection(selected_ids: Vec<String>, interest_id: String) -> Vec<String> {
    let mut selected = selected_interest_set(&selected_ids);
    if !is_known_interest(&interest_id) {
        return selected_ids_for_set(&selected);
    }

    if selected.contains(interest_id.as_str()) {
        selected.remove(interest_id.as_str());
    } else {
        selected.insert(interest_id.as_str());
    }

    selected_ids_for_set(&selected)
}

fn selected_interest_set(selected_ids: &[String]) -> HashSet<&str> {
    let requested: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
    INTERESTS
        .iter()
        .filter_map(|interest| requested.contains(interest.id).then_some(interest.id))
        .collect()
}

fn selected_ids_for_set(selected: &HashSet<&str>) -> Vec<String> {
    INTERESTS
        .iter()
        .filter_map(|interest| {
            selected
                .contains(interest.id)
                .then_some(interest.id.to_string())
        })
        .collect()
}

fn is_known_interest(interest_id: &str) -> bool {
    INTERESTS.iter().any(|interest| interest.id == interest_id)
}

fn interest_selection_for_set(selected: &HashSet<&str>) -> OnboardingInterestSelection {
    let selected_count = selected.len() as u32;
    let remaining = MINIMUM_INTERESTS.saturating_sub(selected_count);
    OnboardingInterestSelection {
        minimum_required: MINIMUM_INTERESTS,
        selected_count,
        remaining,
        can_continue: remaining == 0,
        follow_pubkeys: interest_follow_pubkeys(selected),
    }
}

fn interest_follow_pubkeys(selected: &HashSet<&str>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for interest in INTERESTS {
        if !selected.contains(interest.id) {
            continue;
        }
        for pubkey in interest.pubkeys {
            if seen.insert(*pubkey) {
                out.push((*pubkey).to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_catalog_preserves_ios_chip_order() {
        let interests = interest_catalog();
        assert_eq!(interests.len(), 20);
        assert_eq!(interests[0].id, "philosophy");
        assert_eq!(interests[0].emoji, "🧠");
        assert_eq!(interests[0].label, "Philosophy");
        assert_eq!(interests[19].id, "health");
        assert_eq!(interests[19].label, "Health");
    }

    #[test]
    fn interest_selection_computes_progress_and_deduped_follows() {
        let selection = interest_selection(vec![
            "bitcoin".into(),
            "technology".into(),
            "nostr".into(),
            "technology".into(),
        ]);
        assert_eq!(selection.minimum_required, 3);
        assert_eq!(selection.selected_count, 3);
        assert_eq!(selection.remaining, 0);
        assert!(selection.can_continue);
        assert_eq!(
            selection.follow_pubkeys,
            vec![FIATJAF_PUBKEY.to_string(), JACK_PUBKEY.to_string()]
        );

        let incomplete = interest_selection(vec!["science".into()]);
        assert_eq!(incomplete.remaining, 2);
        assert!(!incomplete.can_continue);
        assert!(incomplete.follow_pubkeys.is_empty());

        let invalid =
            interest_selection(vec!["unknown".into(), "science".into(), "unknown".into()]);
        assert_eq!(invalid.selected_count, 1);
        assert_eq!(invalid.remaining, 2);
    }

    #[test]
    fn interest_projection_marks_selected_chips_and_reuses_selection_policy() {
        let projection = interest_projection(vec![
            "bitcoin".into(),
            "technology".into(),
            "nostr".into(),
            "technology".into(),
        ]);

        assert_eq!(projection.interests.len(), INTERESTS.len());
        let selected_ids: Vec<&str> = projection
            .interests
            .iter()
            .filter(|interest| interest.is_selected)
            .map(|interest| interest.id.as_str())
            .collect();
        assert_eq!(selected_ids, vec!["technology", "bitcoin", "nostr"]);
        assert_eq!(projection.selection.minimum_required, 3);
        assert_eq!(projection.selection.selected_count, 3);
        assert_eq!(projection.selection.remaining, 0);
        assert!(projection.selection.can_continue);
        assert_eq!(
            projection.selection.follow_pubkeys,
            vec![FIATJAF_PUBKEY.to_string(), JACK_PUBKEY.to_string()]
        );
    }

    #[test]
    fn toggle_interest_selection_canonicalizes_and_rejects_unknowns() {
        let selected = toggle_interest_selection(
            vec!["technology".into(), "unknown".into(), "technology".into()],
            "bitcoin".into(),
        );
        assert_eq!(selected, vec!["technology", "bitcoin"]);

        let selected = toggle_interest_selection(selected, "technology".into());
        assert_eq!(selected, vec!["bitcoin"]);

        let selected = toggle_interest_selection(selected, "unknown".into());
        assert_eq!(selected, vec!["bitcoin"]);
    }
}
