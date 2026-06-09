//! Room invite picker projections.
//!
//! Native shells render the chips/rows and execute profile-fetch and
//! add-member side effects. Rust owns query normalization, follow matching,
//! pasted pubkey classification, selected-state derivation, self/duplicate
//! add validation, short labels, and post-send selection/error policy.

use std::collections::{HashMap, HashSet};

use nostr_sdk::prelude::*;

use crate::errors::CoreError;
use crate::models::ProfileMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum RoomInviteCandidateSource {
    Follow,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum RoomInviteInputFormat {
    Npub,
    Nprofile,
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct RoomInviteCandidate {
    pub pubkey_hex: String,
    pub source: RoomInviteCandidateSource,
}

#[derive(Debug, Clone)]
pub struct RoomInviteProjectionContext {
    pub query: String,
    pub follows: Vec<String>,
    pub profiles: Vec<ProfileMetadata>,
    pub selected: Vec<RoomInviteCandidate>,
    pub follows_loaded: bool,
    pub limit: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomInviteProjection {
    pub selected_chips: Vec<RoomInviteChip>,
    pub visible_follows: Vec<RoomInviteSuggestion>,
    pub resolved_candidate: Option<RoomInviteResolvedCandidate>,
    pub show_empty_follow_message: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomInviteSnapshotInput {
    pub query: String,
    pub profiles: Vec<ProfileMetadata>,
    pub selected: Vec<RoomInviteCandidate>,
    pub limit: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomInviteSnapshot {
    pub projection: RoomInviteProjection,
    pub profile_pubkeys_to_request: Vec<String>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomShareLinkSnapshot {
    pub share_url: String,
    pub link_label: String,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteChip {
    pub pubkey_hex: String,
    pub source: RoomInviteCandidateSource,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteSuggestion {
    pub pubkey_hex: String,
    pub source: RoomInviteCandidateSource,
    pub secondary_label: String,
    pub display_name: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteResolvedCandidate {
    pub pubkey_hex: String,
    pub format: RoomInviteInputFormat,
    pub label: String,
    pub source: RoomInviteCandidateSource,
    pub display_name: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RoomInviteAddDecision {
    should_add: bool,
    error_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum RoomInviteSelectionAction {
    Add,
    Toggle,
    Remove,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomInviteSelectionInput {
    pub selected: Vec<RoomInviteCandidate>,
    pub candidate: RoomInviteCandidate,
    pub current_user_pubkey: String,
    pub action: RoomInviteSelectionAction,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteSelectionProjection {
    pub selected: Vec<RoomInviteCandidate>,
    pub error_message: String,
    pub selection_changed: bool,
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct RoomInviteSelectionChromeInput {
    pub selected_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteSelectionChromeProjection {
    pub add_button_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteSendResultProjection {
    pub all_succeeded: bool,
    pub all_failed: bool,
    pub added_count: u64,
    pub success_toast: String,
    pub error_message: String,
    pub remaining_selected: Vec<RoomInviteCandidate>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomInviteAvatarProjectionInput {
    pub pubkey_hex: String,
    pub profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomInviteAvatarProjection {
    pub picture_url: String,
    pub display_initial: String,
}

pub fn project_invite(input: RoomInviteProjectionContext) -> RoomInviteProjection {
    let limit = if input.limit == 0 {
        50usize
    } else {
        input.limit as usize
    };
    let profiles = profile_map(&input.profiles);
    let selected_set = selected_set(&input.selected);
    let normalized_query = normalize_query(&input.query);
    let resolved_candidate = resolved_candidate(&normalized_query, &profiles, &selected_set);
    let visible_follows = if resolved_candidate.is_some() {
        Vec::new()
    } else {
        visible_follows(
            &normalized_query,
            &input.follows,
            &profiles,
            &selected_set,
            limit,
        )
    };
    let show_empty_follow_message = resolved_candidate.is_none()
        && visible_follows.is_empty()
        && !normalized_query.is_empty()
        && input.follows_loaded;

    RoomInviteProjection {
        selected_chips: input
            .selected
            .iter()
            .map(|candidate| RoomInviteChip {
                pubkey_hex: candidate.pubkey_hex.clone(),
                source: candidate.source,
                display_name: chip_display_name(
                    profiles
                        .get(&candidate.pubkey_hex.to_ascii_lowercase())
                        .copied(),
                    &candidate.pubkey_hex,
                ),
            })
            .collect(),
        visible_follows,
        resolved_candidate,
        show_empty_follow_message,
    }
}

pub fn snapshot(
    input: RoomInviteSnapshotInput,
    follows_result: Result<Vec<String>, CoreError>,
) -> RoomInviteSnapshot {
    let (follows, error) = match follows_result {
        Ok(follows) => (follows, String::new()),
        Err(error) => (Vec::new(), error.to_string()),
    };
    let projection = project_invite(RoomInviteProjectionContext {
        query: input.query,
        follows,
        profiles: input.profiles,
        selected: input.selected,
        follows_loaded: true,
        limit: input.limit,
    });

    RoomInviteSnapshot {
        profile_pubkeys_to_request: projection_profile_pubkeys(&projection),
        projection,
        error,
    }
}

pub fn share_link_snapshot(
    group_id: &str,
    codes_result: Result<Vec<String>, CoreError>,
) -> RoomShareLinkSnapshot {
    match codes_result {
        Ok(codes) => {
            let Some(code) = codes.first() else {
                return RoomShareLinkSnapshot {
                    share_url: String::new(),
                    link_label: "Couldn't create invite link".into(),
                    error_message: "No code returned.".into(),
                };
            };
            let group_id = group_id.trim();
            if group_id.is_empty() {
                return RoomShareLinkSnapshot {
                    share_url: String::new(),
                    link_label: "Couldn't create invite link".into(),
                    error_message: "Couldn't mint invite link. Add people directly below.".into(),
                };
            }
            let share_url = format!("https://highlighter.com/r/{group_id}/join/{code}");
            RoomShareLinkSnapshot {
                link_label: share_url.clone(),
                share_url,
                error_message: String::new(),
            }
        }
        Err(_) => RoomShareLinkSnapshot {
            share_url: String::new(),
            link_label: "Couldn't create invite link".into(),
            error_message: "Couldn't mint invite link. Add people directly below.".into(),
        },
    }
}

pub fn avatar_projection(input: RoomInviteAvatarProjectionInput) -> RoomInviteAvatarProjection {
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();
    let display_initial = input
        .profile
        .as_ref()
        .and_then(|profile| profile.name.chars().next())
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_else(|| {
            input
                .pubkey_hex
                .chars()
                .take(1)
                .collect::<String>()
                .to_uppercase()
        });

    RoomInviteAvatarProjection {
        picture_url,
        display_initial,
    }
}

fn add_decision(
    pubkey_hex: &str,
    selected_pubkeys: &[String],
    current_user_pubkey: &str,
) -> RoomInviteAddDecision {
    if selected_pubkeys
        .iter()
        .any(|selected| selected.eq_ignore_ascii_case(pubkey_hex))
    {
        return RoomInviteAddDecision {
            should_add: false,
            error_message: String::new(),
        };
    }

    if !current_user_pubkey.is_empty() && pubkey_hex.eq_ignore_ascii_case(current_user_pubkey) {
        return RoomInviteAddDecision {
            should_add: false,
            error_message: "You're already in this room.".into(),
        };
    }

    RoomInviteAddDecision {
        should_add: true,
        error_message: String::new(),
    }
}

pub fn project_selection(input: RoomInviteSelectionInput) -> RoomInviteSelectionProjection {
    let mut selected = input.selected;
    let candidate = input.candidate;
    let candidate_key = candidate.pubkey_hex.to_ascii_lowercase();
    let selected_pubkeys = selected
        .iter()
        .map(|candidate| candidate.pubkey_hex.clone())
        .collect::<Vec<_>>();
    let selected_index = selected.iter().position(|selected| {
        selected
            .pubkey_hex
            .eq_ignore_ascii_case(&candidate.pubkey_hex)
    });

    match input.action {
        RoomInviteSelectionAction::Remove => {
            if let Some(index) = selected_index {
                selected.remove(index);
                RoomInviteSelectionProjection {
                    selected,
                    error_message: String::new(),
                    selection_changed: true,
                }
            } else {
                RoomInviteSelectionProjection {
                    selected,
                    error_message: String::new(),
                    selection_changed: false,
                }
            }
        }
        RoomInviteSelectionAction::Toggle if selected_index.is_some() => {
            selected.remove(selected_index.expect("checked"));
            RoomInviteSelectionProjection {
                selected,
                error_message: String::new(),
                selection_changed: true,
            }
        }
        RoomInviteSelectionAction::Add | RoomInviteSelectionAction::Toggle => {
            let decision = add_decision(
                &candidate_key,
                &selected_pubkeys,
                &input.current_user_pubkey,
            );
            if !decision.should_add {
                return RoomInviteSelectionProjection {
                    selected,
                    error_message: decision.error_message,
                    selection_changed: false,
                };
            }
            selected.push(RoomInviteCandidate {
                pubkey_hex: candidate_key,
                source: candidate.source,
            });
            RoomInviteSelectionProjection {
                selected,
                error_message: String::new(),
                selection_changed: true,
            }
        }
    }
}

pub fn project_selection_chrome(
    input: RoomInviteSelectionChromeInput,
) -> RoomInviteSelectionChromeProjection {
    RoomInviteSelectionChromeProjection {
        add_button_label: invite_add_button_label(input.selected_count),
    }
}

pub fn project_send_result(
    selected: &[RoomInviteCandidate],
    failed_pubkeys: &[String],
) -> RoomInviteSendResultProjection {
    let failed_set: HashSet<String> = failed_pubkeys
        .iter()
        .map(|pubkey| pubkey.to_ascii_lowercase())
        .collect();
    let remaining_selected = selected
        .iter()
        .filter(|candidate| failed_set.contains(&candidate.pubkey_hex.to_ascii_lowercase()))
        .cloned()
        .collect::<Vec<_>>();
    let failed_labels = selected
        .iter()
        .filter(|candidate| failed_set.contains(&candidate.pubkey_hex.to_ascii_lowercase()))
        .map(|candidate| short_pubkey(&candidate.pubkey_hex))
        .collect::<Vec<_>>();
    let failed_count = remaining_selected.len();
    let added_count = selected.len().saturating_sub(failed_count) as u64;

    RoomInviteSendResultProjection {
        all_succeeded: failed_count == 0,
        all_failed: !selected.is_empty() && failed_count == selected.len(),
        added_count,
        success_toast: if added_count == 1 {
            "Added 1 person".into()
        } else {
            format!("Added {added_count} people")
        },
        error_message: if failed_count == selected.len() {
            "Couldn't add anyone. Are you a moderator of this room?".into()
        } else if failed_count > 0 {
            format!("Some failed: {}", failed_labels.join(", "))
        } else {
            String::new()
        },
        remaining_selected,
    }
}

pub fn project_send_error(
    selected: &[RoomInviteCandidate],
    error: CoreError,
) -> RoomInviteSendResultProjection {
    RoomInviteSendResultProjection {
        all_succeeded: false,
        all_failed: !selected.is_empty(),
        added_count: 0,
        success_toast: String::new(),
        error_message: error.to_string(),
        remaining_selected: selected.to_vec(),
    }
}

fn invite_add_button_label(count: u64) -> String {
    if count == 1 {
        "Add 1 person".into()
    } else {
        format!("Add {count} people")
    }
}

pub fn decode_pubkey_reference(input: &str) -> Result<(String, RoomInviteInputFormat), CoreError> {
    let trimmed = normalize_query(input);
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput("empty pubkey reference".into()));
    }

    if let Ok(pk) = PublicKey::from_bech32(&trimmed) {
        return Ok((pk.to_hex(), RoomInviteInputFormat::Npub));
    }
    if let Ok(profile) = nostr_sdk::nips::nip19::Nip19Profile::from_bech32(&trimmed) {
        return Ok((profile.public_key.to_hex(), RoomInviteInputFormat::Nprofile));
    }
    if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok((trimmed.to_ascii_lowercase(), RoomInviteInputFormat::Hex));
    }
    Err(CoreError::InvalidInput(format!(
        "unrecognised pubkey reference: {trimmed}"
    )))
}

pub fn short_pubkey(hex: &str) -> String {
    if hex.chars().count() <= 12 {
        return hex.to_string();
    }
    let prefix = hex.chars().take(6).collect::<String>();
    let suffix = hex
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}…{suffix}")
}

fn visible_follows(
    query: &str,
    follows: &[String],
    profiles: &HashMap<String, &ProfileMetadata>,
    selected_set: &HashSet<String>,
    limit: usize,
) -> Vec<RoomInviteSuggestion> {
    let follows_iter = follows.iter().filter(|pubkey| {
        query.is_empty()
            || profiles
                .get(&pubkey.to_ascii_lowercase())
                .map(|profile| profile_matches(profile, query))
                .unwrap_or(false)
    });

    follows_iter
        .take(limit)
        .map(|pubkey| {
            let profile = profiles.get(&pubkey.to_ascii_lowercase()).copied();
            RoomInviteSuggestion {
                pubkey_hex: pubkey.clone(),
                source: RoomInviteCandidateSource::Follow,
                secondary_label: "Following".into(),
                display_name: row_display_name(profile, pubkey),
                is_selected: selected_set.contains(&pubkey.to_ascii_lowercase()),
            }
        })
        .collect()
}

fn resolved_candidate(
    query: &str,
    profiles: &HashMap<String, &ProfileMetadata>,
    selected_set: &HashSet<String>,
) -> Option<RoomInviteResolvedCandidate> {
    if !looks_like_reference(query) {
        return None;
    }

    let Ok((pubkey_hex, format)) = decode_pubkey_reference(query) else {
        return None;
    };
    let profile = profiles.get(&pubkey_hex.to_ascii_lowercase()).copied();
    Some(RoomInviteResolvedCandidate {
        pubkey_hex: pubkey_hex.clone(),
        format,
        label: input_format_label(format),
        source: RoomInviteCandidateSource::Paste,
        display_name: row_display_name(profile, &pubkey_hex),
        is_selected: selected_set.contains(&pubkey_hex.to_ascii_lowercase()),
    })
}

fn input_format_label(format: RoomInviteInputFormat) -> String {
    match format {
        RoomInviteInputFormat::Npub => "Pasted npub",
        RoomInviteInputFormat::Nprofile => "Pasted nprofile",
        RoomInviteInputFormat::Hex => "Pasted pubkey",
    }
    .into()
}

fn profile_matches(profile: &ProfileMetadata, needle: &str) -> bool {
    profile.name.to_ascii_lowercase().contains(needle)
        || profile.nip05.to_ascii_lowercase().contains(needle)
        || profile.display_name.to_ascii_lowercase().contains(needle)
}

fn row_display_name(profile: Option<&ProfileMetadata>, fallback_hex: &str) -> String {
    if let Some(profile) = profile {
        if !profile.display_name.is_empty() {
            return profile.display_name.clone();
        }
        if !profile.name.is_empty() {
            return profile.name.clone();
        }
    }
    short_pubkey(fallback_hex)
}

fn chip_display_name(profile: Option<&ProfileMetadata>, fallback_hex: &str) -> String {
    if let Some(profile) = profile {
        if !profile.name.is_empty() {
            return profile.name.clone();
        }
    }
    fallback_hex.chars().take(8).collect()
}

fn profile_map(profiles: &[ProfileMetadata]) -> HashMap<String, &ProfileMetadata> {
    profiles
        .iter()
        .map(|profile| (profile.pubkey.to_ascii_lowercase(), profile))
        .collect()
}

fn selected_set(selected: &[RoomInviteCandidate]) -> HashSet<String> {
    selected
        .iter()
        .map(|candidate| candidate.pubkey_hex.to_ascii_lowercase())
        .collect()
}

fn projection_profile_pubkeys(projection: &RoomInviteProjection) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut pubkeys = Vec::new();

    for chip in &projection.selected_chips {
        push_unique_pubkey(&mut pubkeys, &mut seen, &chip.pubkey_hex);
    }
    for row in &projection.visible_follows {
        push_unique_pubkey(&mut pubkeys, &mut seen, &row.pubkey_hex);
    }
    if let Some(resolved) = &projection.resolved_candidate {
        push_unique_pubkey(&mut pubkeys, &mut seen, &resolved.pubkey_hex);
    }

    pubkeys
}

fn push_unique_pubkey(pubkeys: &mut Vec<String>, seen: &mut HashSet<String>, pubkey: &str) {
    if seen.insert(pubkey.to_ascii_lowercase()) {
        pubkeys.push(pubkey.to_string());
    }
}

fn normalize_query(input: &str) -> String {
    input.trim().replace("nostr:", "")
}

fn looks_like_reference(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    (lower.starts_with("npub1") && lower.chars().count() >= 60)
        || (lower.starts_with("nprofile1") && lower.chars().count() >= 60)
        || (input.chars().count() == 64 && input.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_invite_filters_follows_by_profile_and_marks_selection() {
        let follows = vec![hex("01"), hex("02"), hex("03")];
        let selected = vec![RoomInviteCandidate {
            pubkey_hex: hex("02"),
            source: RoomInviteCandidateSource::Follow,
        }];

        let projection = project_invite(RoomInviteProjectionContext {
            query: "ada".into(),
            follows: follows.clone(),
            profiles: vec![
                profile(&follows[0], "grace", "", "grace@example.com"),
                profile(&follows[1], "ada", "Ada Lovelace", "ada@example.com"),
                profile(&follows[2], "linus", "", "linus@example.com"),
            ],
            selected: selected.clone(),
            follows_loaded: true,
            limit: 50,
        });

        assert_eq!(projection.resolved_candidate, None);
        assert!(!projection.show_empty_follow_message);
        assert_eq!(projection.visible_follows.len(), 1);
        assert_eq!(projection.visible_follows[0].pubkey_hex, follows[1]);
        assert_eq!(projection.visible_follows[0].display_name, "Ada Lovelace");
        assert!(projection.visible_follows[0].is_selected);
        assert_eq!(projection.selected_chips.len(), 1);
        assert_eq!(projection.selected_chips[0].display_name, "ada");
    }

    #[test]
    fn project_invite_resolves_hex_paste_and_hides_follow_empty_message() {
        let pasted = hex("0a");
        let projection = project_invite(RoomInviteProjectionContext {
            query: format!(" nostr:{pasted} "),
            follows: Vec::new(),
            profiles: vec![profile(&pasted, "", "Paste User", "")],
            selected: Vec::new(),
            follows_loaded: true,
            limit: 50,
        });

        let resolved = projection.resolved_candidate.expect("resolved");
        assert_eq!(resolved.pubkey_hex, pasted);
        assert_eq!(resolved.format, RoomInviteInputFormat::Hex);
        assert_eq!(resolved.label, "Pasted pubkey");
        assert_eq!(resolved.display_name, "Paste User");
        assert!(projection.visible_follows.is_empty());
        assert!(!projection.show_empty_follow_message);
    }

    #[test]
    fn project_invite_caps_blank_follows_and_shows_empty_query_message() {
        let follows = (0..60).map(|idx| format!("{idx:064x}")).collect::<Vec<_>>();

        let blank = project_invite(RoomInviteProjectionContext {
            query: String::new(),
            follows: follows.clone(),
            profiles: Vec::new(),
            selected: Vec::new(),
            follows_loaded: true,
            limit: 50,
        });
        assert_eq!(blank.visible_follows.len(), 50);
        assert!(!blank.show_empty_follow_message);

        let empty = project_invite(RoomInviteProjectionContext {
            query: "missing".into(),
            follows,
            profiles: Vec::new(),
            selected: Vec::new(),
            follows_loaded: true,
            limit: 50,
        });
        assert!(empty.visible_follows.is_empty());
        assert!(empty.show_empty_follow_message);
    }

    #[test]
    fn snapshot_projects_cached_follows_and_profile_requests_without_exposing_follow_list() {
        let follows = vec![hex("01"), hex("02"), hex("03")];
        let selected = vec![RoomInviteCandidate {
            pubkey_hex: follows[1].clone(),
            source: RoomInviteCandidateSource::Follow,
        }];

        let snapshot = snapshot(
            RoomInviteSnapshotInput {
                query: String::new(),
                profiles: vec![profile(&follows[0], "grace", "Grace Hopper", "")],
                selected: selected.clone(),
                limit: 2,
            },
            Ok(follows.clone()),
        );

        assert!(snapshot.error.is_empty());
        assert_eq!(snapshot.projection.visible_follows.len(), 2);
        assert_eq!(snapshot.projection.selected_chips.len(), 1);
        assert_eq!(snapshot.projection.selected_chips[0].pubkey_hex, follows[1]);
        assert_eq!(
            snapshot.profile_pubkeys_to_request,
            vec![follows[1].clone(), follows[0].clone()]
        );
    }

    #[test]
    fn snapshot_surfaces_follow_lookup_errors_in_state() {
        let snapshot = snapshot(
            RoomInviteSnapshotInput {
                query: "ada".into(),
                profiles: Vec::new(),
                selected: Vec::new(),
                limit: 50,
            },
            Err(CoreError::NotAuthenticated),
        );

        assert_eq!(snapshot.error, "not authenticated");
        assert!(snapshot.projection.visible_follows.is_empty());
        assert!(snapshot.projection.show_empty_follow_message);
    }

    #[test]
    fn share_link_snapshot_projects_url_empty_and_error_states() {
        let success = share_link_snapshot("room", Ok(vec!["code123".into()]));
        let empty = share_link_snapshot("room", Ok(Vec::new()));
        let failure = share_link_snapshot("room", Err(CoreError::Relay("nope".into())));

        assert_eq!(
            success.share_url,
            "https://highlighter.com/r/room/join/code123"
        );
        assert_eq!(success.link_label, success.share_url);
        assert!(success.error_message.is_empty());
        assert!(empty.share_url.is_empty());
        assert_eq!(empty.link_label, "Couldn't create invite link");
        assert_eq!(empty.error_message, "No code returned.");
        assert!(failure.share_url.is_empty());
        assert_eq!(
            failure.error_message,
            "Couldn't mint invite link. Add people directly below."
        );
    }

    #[test]
    fn avatar_projection_prefers_profile_name_initial() {
        let pubkey = hex("01");
        let projection = avatar_projection(RoomInviteAvatarProjectionInput {
            pubkey_hex: pubkey.clone(),
            profile: Some(profile(&pubkey, "ada", "Ada Lovelace", "ada@example.com")),
        });

        assert_eq!(
            projection,
            RoomInviteAvatarProjection {
                picture_url: String::new(),
                display_initial: "A".into(),
            }
        );
    }

    #[test]
    fn avatar_projection_falls_back_to_pubkey_initial() {
        let projection = avatar_projection(RoomInviteAvatarProjectionInput {
            pubkey_hex: "fbcdef".into(),
            profile: Some(profile(&hex("01"), "", "Ada Lovelace", "ada@example.com")),
        });

        assert_eq!(
            projection,
            RoomInviteAvatarProjection {
                picture_url: String::new(),
                display_initial: "F".into(),
            }
        );
    }

    #[test]
    fn add_decision_rejects_duplicates_and_self() {
        let selected = vec![hex("01")];
        assert_eq!(
            add_decision(&hex("01"), &selected, ""),
            RoomInviteAddDecision {
                should_add: false,
                error_message: String::new()
            }
        );
        assert_eq!(
            add_decision(&hex("02"), &selected, &hex("02")),
            RoomInviteAddDecision {
                should_add: false,
                error_message: "You're already in this room.".into()
            }
        );
        assert!(add_decision(&hex("03"), &selected, &hex("02")).should_add);
    }

    #[test]
    fn selection_reducer_adds_toggles_removes_and_rejects_self() {
        let current_user = hex("09");
        let first = candidate("01", RoomInviteCandidateSource::Follow);

        let added = project_selection(RoomInviteSelectionInput {
            selected: Vec::new(),
            candidate: first.clone(),
            current_user_pubkey: current_user.clone(),
            action: RoomInviteSelectionAction::Add,
        });
        assert!(added.selection_changed);
        assert_eq!(added.selected, vec![first.clone()]);
        assert!(added.error_message.is_empty());

        let duplicate = project_selection(RoomInviteSelectionInput {
            selected: added.selected.clone(),
            candidate: first.clone(),
            current_user_pubkey: current_user.clone(),
            action: RoomInviteSelectionAction::Add,
        });
        assert!(!duplicate.selection_changed);
        assert_eq!(duplicate.selected, vec![first.clone()]);
        assert!(duplicate.error_message.is_empty());

        let toggled_off = project_selection(RoomInviteSelectionInput {
            selected: added.selected.clone(),
            candidate: first.clone(),
            current_user_pubkey: current_user.clone(),
            action: RoomInviteSelectionAction::Toggle,
        });
        assert!(toggled_off.selection_changed);
        assert!(toggled_off.selected.is_empty());

        let self_add = project_selection(RoomInviteSelectionInput {
            selected: Vec::new(),
            candidate: RoomInviteCandidate {
                pubkey_hex: current_user.clone(),
                source: RoomInviteCandidateSource::Paste,
            },
            current_user_pubkey: current_user,
            action: RoomInviteSelectionAction::Add,
        });
        assert!(!self_add.selection_changed);
        assert!(self_add.selected.is_empty());
        assert_eq!(self_add.error_message, "You're already in this room.");

        let removed = project_selection(RoomInviteSelectionInput {
            selected: added.selected,
            candidate: first,
            current_user_pubkey: String::new(),
            action: RoomInviteSelectionAction::Remove,
        });
        assert!(removed.selection_changed);
        assert!(removed.selected.is_empty());
    }

    #[test]
    fn selection_chrome_projects_add_button_label() {
        let one = project_selection_chrome(RoomInviteSelectionChromeInput { selected_count: 1 });
        assert_eq!(one.add_button_label, "Add 1 person");

        let many = project_selection_chrome(RoomInviteSelectionChromeInput { selected_count: 3 });
        assert_eq!(many.add_button_label, "Add 3 people");
    }

    #[test]
    fn send_result_projects_success_partial_and_total_failure() {
        let selected = vec![
            candidate("01", RoomInviteCandidateSource::Follow),
            candidate("02", RoomInviteCandidateSource::Paste),
        ];

        let success = project_send_result(&selected, &[]);
        assert!(success.all_succeeded);
        assert_eq!(success.added_count, 2);
        assert_eq!(success.success_toast, "Added 2 people");
        assert!(success.remaining_selected.is_empty());

        let partial = project_send_result(&selected, &[hex("02")]);
        assert!(!partial.all_succeeded);
        assert!(!partial.all_failed);
        assert_eq!(partial.added_count, 1);
        assert_eq!(
            partial.error_message,
            format!("Some failed: {}", short_pubkey(&hex("02")))
        );
        assert_eq!(partial.remaining_selected, vec![selected[1].clone()]);

        let total = project_send_result(&selected, &[hex("01"), hex("02")]);
        assert!(total.all_failed);
        assert_eq!(
            total.error_message,
            "Couldn't add anyone. Are you a moderator of this room?"
        );
    }

    #[test]
    fn send_error_projection_preserves_selection_and_surfaces_error() {
        let selected = vec![candidate("01", RoomInviteCandidateSource::Follow)];
        let projection = project_send_error(&selected, CoreError::NotAuthenticated);

        assert!(!projection.all_succeeded);
        assert!(projection.all_failed);
        assert_eq!(projection.added_count, 0);
        assert!(projection.success_toast.is_empty());
        assert_eq!(projection.error_message, "not authenticated");
        assert_eq!(projection.remaining_selected, selected);
    }

    fn candidate(suffix: &str, source: RoomInviteCandidateSource) -> RoomInviteCandidate {
        RoomInviteCandidate {
            pubkey_hex: hex(suffix),
            source,
        }
    }

    fn profile(pubkey: &str, name: &str, display_name: &str, nip05: &str) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: pubkey.into(),
            name: name.into(),
            display_name: display_name.into(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: nip05.into(),
            website: String::new(),
            lud16: String::new(),
            created_at: Some(1),
        }
    }

    fn hex(suffix: &str) -> String {
        format!("{:0>64}", suffix)
    }
}
