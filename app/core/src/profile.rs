//! NIP-01 kind:0 profile metadata query. The profile view reads from nostrdb
//! first; the relay-side hydrate happens via
//! `SubscriptionKind::UserProfile` so stale cache rows get refreshed while
//! the view is open.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::errors::CoreError;
use crate::models::{ProfileMetadata, ProfileUpdateDraft};
use crate::nostr_runtime::{mirror_social_trio_to_purple, NostrRuntime};

const KIND_METADATA: u16 = 0;

/// Read the newest kind:0 event for `pubkey_hex` out of nostrdb and parse its
/// JSON content into a `ProfileMetadata`. Returns `None` when no metadata is
/// cached yet; the caller can still render a pubkey-only view while the
/// subscription fills in.
pub fn query_profile_from_ndb(
    ndb: &Ndb,
    pubkey_hex: &str,
) -> Result<Option<ProfileMetadata>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_METADATA as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query profile: {e}")))?;

    // Nostrdb may return several kind:0s for this pubkey if relays delivered
    // older revisions. Keep the newest by `created_at`.
    let mut newest: Option<Event> = None;
    for result in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, result.note_key) else {
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
        return Ok(None);
    };
    Ok(Some(parse_metadata(&event)))
}

/// Pure: parse a kind:0 event into a `ProfileMetadata`. Unknown fields are
/// silently dropped; a completely unparseable body yields a record with only
/// the pubkey populated so the view still has something to render.
pub fn parse_metadata(event: &Event) -> ProfileMetadata {
    let pubkey = event.pubkey.to_hex();
    let created_at = Some(event.created_at.as_secs());
    let raw: RawMetadata = serde_json::from_str(&event.content).unwrap_or_default();

    ProfileMetadata {
        pubkey,
        name: raw.name.unwrap_or_default().trim().to_string(),
        display_name: raw
            .display_name
            .or(raw.displayname)
            .unwrap_or_default()
            .trim()
            .to_string(),
        about: raw.about.unwrap_or_default().trim().to_string(),
        picture: raw
            .picture
            .or(raw.image)
            .unwrap_or_default()
            .trim()
            .to_string(),
        banner: raw.banner.unwrap_or_default().trim().to_string(),
        nip05: raw.nip05.unwrap_or_default().trim().to_string(),
        website: raw.website.unwrap_or_default().trim().to_string(),
        lud16: raw.lud16.unwrap_or_default().trim().to_string(),
        created_at,
    }
}

/// JSON shape of the kind:0 content blob. Tolerates both `display_name` (spec)
/// and `displayName` / `displayname` (seen in the wild). Missing fields stay
/// `None`.
#[derive(Debug, Default, Deserialize)]
struct RawMetadata {
    name: Option<String>,
    #[serde(alias = "displayName")]
    display_name: Option<String>,
    displayname: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    image: Option<String>,
    banner: Option<String>,
    nip05: Option<String>,
    website: Option<String>,
    lud16: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileDisplayProjectionInput {
    pub pubkey: String,
    pub profile: Option<ProfileMetadata>,
    pub fallback: ProfileDisplayFallback,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileDisplayWithLabelProjectionInput {
    pub pubkey: String,
    pub profile: Option<ProfileMetadata>,
    pub label_fallback: String,
    pub pubkey_fallback: ProfileDisplayFallback,
    pub empty_fallback: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ProfileDisplayFallback {
    Pubkey8,
    Pubkey10,
    Pubkey12,
    AccountLabel,
    Pubkey6,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProfileDisplayProjection {
    pub display_name: String,
    pub display_initial: String,
    pub picture_url: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileIdentityProjectionInput {
    pub pubkey: String,
    pub profile: Option<ProfileMetadata>,
    pub fallback: ProfileDisplayFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProfileIdentityProjection {
    pub display_name: String,
    pub display_initial: String,
    pub picture_url: String,
    pub bio: String,
    pub verified_nip05: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileRelationshipProjectionInput {
    pub profile_pubkey: String,
    pub viewer_pubkey: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProfileRelationshipProjection {
    pub target_pubkey: String,
    pub is_own_profile: bool,
    pub can_show_follow_action: bool,
    pub should_refresh_follow_state: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileUpdateProjectionInput {
    pub initial: Option<ProfileMetadata>,
    pub name: String,
    pub display_name: String,
    pub about: String,
    pub picture: String,
    pub banner: String,
    pub nip05: String,
    pub website: String,
    pub lud16: String,
    pub saving: bool,
    pub picture_uploading: bool,
    pub banner_uploading: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileUpdateProjection {
    pub draft: ProfileUpdateDraft,
    pub is_dirty: bool,
    pub can_save: bool,
}

/// Pure profile presentation projection. Rust owns profile-name precedence,
/// pubkey fallback, and avatar source selection; native shells only render.
pub fn profile_display_projection(
    input: ProfileDisplayProjectionInput,
) -> ProfileDisplayProjection {
    let display_name = match input.profile.as_ref() {
        Some(profile) if !profile.display_name.is_empty() => profile.display_name.clone(),
        Some(profile) if !profile.name.is_empty() => profile.name.clone(),
        _ => profile_display_fallback_name(&input.pubkey, input.fallback),
    };
    let display_initial = match input.profile.as_ref() {
        Some(profile) if !profile.display_name.is_empty() => {
            profile.display_name.chars().take(1).collect()
        }
        Some(profile) if !profile.name.is_empty() => profile.name.chars().take(1).collect(),
        _ => profile_display_fallback_initial(&input.pubkey, input.fallback),
    };
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();

    ProfileDisplayProjection {
        display_name,
        display_initial,
        picture_url,
    }
}

/// Profile/avatar projection for artifact bylines that carry their own author
/// label. Rust owns the precedence: profile display name, profile name,
/// supplied label, pubkey fallback, then empty fallback.
pub fn profile_display_with_label_projection(
    input: ProfileDisplayWithLabelProjectionInput,
) -> ProfileDisplayProjection {
    let label_fallback = input.label_fallback.trim();
    let empty_fallback = input.empty_fallback.trim();
    let has_pubkey = !input.pubkey.trim().is_empty();

    let display_name = match input.profile.as_ref() {
        Some(profile) if !profile.display_name.is_empty() => profile.display_name.clone(),
        Some(profile) if !profile.name.is_empty() => profile.name.clone(),
        _ if !label_fallback.is_empty() => label_fallback.to_string(),
        _ if has_pubkey => profile_display_fallback_name(&input.pubkey, input.pubkey_fallback),
        _ => empty_fallback.to_string(),
    };
    let display_initial = match input.profile.as_ref() {
        Some(profile) if !profile.display_name.is_empty() => {
            profile.display_name.chars().take(1).collect()
        }
        Some(profile) if !profile.name.is_empty() => profile.name.chars().take(1).collect(),
        _ if !label_fallback.is_empty() => label_fallback.chars().take(1).collect(),
        _ if has_pubkey => profile_display_fallback_initial(&input.pubkey, input.pubkey_fallback),
        _ => empty_fallback.chars().take(1).collect(),
    };
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();

    ProfileDisplayProjection {
        display_name,
        display_initial,
        picture_url,
    }
}

/// Profile handle projection for compact social proof. Rust owns handle
/// precedence and preserves the existing pubkey-derived avatar fallback.
pub fn profile_handle_projection(input: ProfileDisplayProjectionInput) -> ProfileDisplayProjection {
    let display_name = match input.profile.as_ref() {
        Some(profile) if !profile.name.is_empty() => profile.name.clone(),
        Some(profile) if !profile.display_name.is_empty() => profile.display_name.clone(),
        _ => profile_display_fallback_name(&input.pubkey, input.fallback),
    };
    let display_initial = profile_display_fallback_initial(&input.pubkey, input.fallback);
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();

    ProfileDisplayProjection {
        display_name,
        display_initial,
        picture_url,
    }
}

/// Profile header identity projection. Rust owns profile display fallback and
/// NIP-05 label normalization; native shells render and execute OS links.
pub fn profile_identity_projection(
    input: ProfileIdentityProjectionInput,
) -> ProfileIdentityProjection {
    let display = profile_display_projection(ProfileDisplayProjectionInput {
        pubkey: input.pubkey,
        profile: input.profile.clone(),
        fallback: input.fallback,
    });
    let bio = input
        .profile
        .as_ref()
        .map(|profile| profile.about.clone())
        .unwrap_or_default();
    let verified_nip05 = input
        .profile
        .as_ref()
        .and_then(|profile| profile_verified_nip05_label(&profile.nip05));

    ProfileIdentityProjection {
        display_name: display.display_name,
        display_initial: display.display_initial,
        picture_url: display.picture_url,
        bio,
        verified_nip05,
    }
}

/// Profile relationship projection. Rust owns pubkey normalization, own-profile
/// detection, and whether native shells should show or refresh follow state.
pub fn profile_relationship_projection(
    input: ProfileRelationshipProjectionInput,
) -> ProfileRelationshipProjection {
    let target_pubkey = input.profile_pubkey.trim().to_string();
    let viewer_pubkey = input
        .viewer_pubkey
        .as_deref()
        .map(str::trim)
        .filter(|viewer| !viewer.is_empty());
    let has_target = !target_pubkey.is_empty();
    let is_own_profile = viewer_pubkey
        .is_some_and(|viewer| has_target && viewer.eq_ignore_ascii_case(&target_pubkey));
    let can_show_follow_action = viewer_pubkey.is_some() && has_target && !is_own_profile;

    ProfileRelationshipProjection {
        target_pubkey,
        is_own_profile,
        can_show_follow_action,
        should_refresh_follow_state: can_show_follow_action,
    }
}

fn profile_display_fallback_name(pubkey: &str, fallback: ProfileDisplayFallback) -> String {
    match fallback {
        ProfileDisplayFallback::Pubkey6 => pubkey.chars().take(6).collect(),
        ProfileDisplayFallback::Pubkey8 => pubkey.chars().take(8).collect(),
        ProfileDisplayFallback::Pubkey10 => pubkey.chars().take(10).collect(),
        ProfileDisplayFallback::Pubkey12 => pubkey.chars().take(12).collect(),
        ProfileDisplayFallback::AccountLabel => "Nostr Account".to_string(),
    }
}

fn profile_display_fallback_initial(pubkey: &str, fallback: ProfileDisplayFallback) -> String {
    match fallback {
        ProfileDisplayFallback::Pubkey6
        | ProfileDisplayFallback::Pubkey8
        | ProfileDisplayFallback::Pubkey10
        | ProfileDisplayFallback::Pubkey12 => pubkey.chars().take(1).collect(),
        ProfileDisplayFallback::AccountLabel => String::new(),
    }
}

fn profile_verified_nip05_label(raw: &str) -> Option<String> {
    if raw.is_empty() {
        None
    } else if let Some(root_label) = raw.strip_prefix("_@") {
        Some(root_label.to_string())
    } else {
        Some(raw.to_string())
    }
}

/// Profile edit-form projection. Rust owns draft normalization and save
/// eligibility; native shells only bind text fields and render controls.
pub fn profile_update_projection(input: ProfileUpdateProjectionInput) -> ProfileUpdateProjection {
    let is_dirty = profile_update_raw_is_dirty(&input);
    let can_save = is_dirty && !input.saving && !input.picture_uploading && !input.banner_uploading;
    let draft = ProfileUpdateDraft {
        name: input.name.trim().to_string(),
        display_name: input.display_name.trim().to_string(),
        about: input.about.trim().to_string(),
        picture: input.picture.trim().to_string(),
        banner: input.banner.trim().to_string(),
        nip05: input.nip05.trim().to_string(),
        website: input.website.trim().to_string(),
        lud16: input.lud16.trim().to_string(),
    };

    ProfileUpdateProjection {
        draft,
        is_dirty,
        can_save,
    }
}

fn profile_update_raw_is_dirty(input: &ProfileUpdateProjectionInput) -> bool {
    let initial = input.initial.as_ref();
    input.display_name != initial.map(|p| p.display_name.as_str()).unwrap_or("")
        || input.name != initial.map(|p| p.name.as_str()).unwrap_or("")
        || input.about != initial.map(|p| p.about.as_str()).unwrap_or("")
        || input.picture != initial.map(|p| p.picture.as_str()).unwrap_or("")
        || input.banner != initial.map(|p| p.banner.as_str()).unwrap_or("")
        || input.nip05 != initial.map(|p| p.nip05.as_str()).unwrap_or("")
        || input.website != initial.map(|p| p.website.as_str()).unwrap_or("")
        || input.lud16 != initial.map(|p| p.lud16.as_str()).unwrap_or("")
}

/// Publish a fresh kind:0 metadata event for the current user. Preserves
/// any unknown fields the user may have set from another client (e.g.
/// `pronouns`, `bot`, `picture_animated`) — we deserialise the existing
/// content as a JSON object and overwrite only the canonical fields the
/// edit form drives. Falls back to a brand-new object if no kind:0 is
/// cached.
///
/// After the standard `send_event` broadcast, mirrors to
/// `purple_pages_relay()` so the canonical social-trio store always has
/// the latest revision (other Nostr clients look there for kind:0).
/// Returns the parsed `ProfileMetadata` so the caller's UI can swap to
/// the new state without waiting for the relay echo.
pub async fn publish_profile(
    runtime: &NostrRuntime,
    draft: &ProfileUpdateDraft,
) -> Result<ProfileMetadata, CoreError> {
    // Recover the current user's pubkey from the active signer so we can
    // load their existing kind:0 from cache.
    let client = runtime.client();
    let signer = client
        .signer()
        .await
        .map_err(|e| CoreError::Signer(format!("get signer: {e}")))?;
    let user_pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| CoreError::Signer(format!("get pubkey: {e}")))?;

    // Start from any existing JSON so unknown keys round-trip.
    let mut content: Value = match query_raw_metadata_json(runtime.ndb(), &user_pubkey.to_hex())? {
        Some(v) if v.is_object() => v,
        _ => json!({}),
    };
    let obj = content
        .as_object_mut()
        .expect("guaranteed to be a JSON object");

    set_or_clear(obj, "name", &draft.name);
    set_or_clear(obj, "display_name", &draft.display_name);
    set_or_clear(obj, "about", &draft.about);
    set_or_clear(obj, "picture", &draft.picture);
    set_or_clear(obj, "banner", &draft.banner);
    set_or_clear(obj, "nip05", &draft.nip05);
    set_or_clear(obj, "website", &draft.website);
    set_or_clear(obj, "lud16", &draft.lud16);

    let body = serde_json::to_string(&content)
        .map_err(|e| CoreError::Other(format!("serialise metadata: {e}")))?;

    let builder = EventBuilder::new(Kind::Custom(KIND_METADATA), body);
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign metadata: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish metadata: {e}")))?;
    mirror_social_trio_to_purple(client, &event).await;

    Ok(parse_metadata(&event))
}

/// Set `key` to `value` if non-empty (after trim), otherwise remove the
/// key entirely. Removing rather than writing `""` keeps a cleared field
/// from re-appearing as a stale empty string on clients that just check
/// for key presence.
fn set_or_clear(obj: &mut serde_json::Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        obj.remove(key);
    } else {
        obj.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

/// Newest cached kind:0 for `pubkey_hex`, parsed as a JSON value (so the
/// caller can preserve unknown fields). `None` when no kind:0 is cached.
fn query_raw_metadata_json(ndb: &Ndb, pubkey_hex: &str) -> Result<Option<Value>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_METADATA as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query profile: {e}")))?;
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
    let Some(event) = newest else { return Ok(None) };
    Ok(serde_json::from_str::<Value>(&event.content).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_metadata(keys: &Keys, json: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_METADATA), json)
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn parses_standard_fields() {
        let keys = Keys::generate();
        let event = sign_metadata(
            &keys,
            r#"{
                "name": "alice",
                "display_name": "Alice Smith",
                "about": " hey ",
                "picture": "https://x/p.png",
                "banner": "https://x/b.png",
                "nip05": "alice@x",
                "website": "https://x",
                "lud16": "alice@x"
            }"#,
        );
        let p = parse_metadata(&event);
        assert_eq!(p.name, "alice");
        assert_eq!(p.display_name, "Alice Smith");
        assert_eq!(p.about, "hey");
        assert_eq!(p.picture, "https://x/p.png");
        assert_eq!(p.banner, "https://x/b.png");
        assert_eq!(p.nip05, "alice@x");
        assert_eq!(p.website, "https://x");
        assert_eq!(p.lud16, "alice@x");
    }

    #[test]
    fn falls_back_from_display_name_alias() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, r#"{"displayName": "CamelCase"}"#);
        let p = parse_metadata(&event);
        assert_eq!(p.display_name, "CamelCase");
    }

    #[test]
    fn image_substitutes_for_missing_picture() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, r#"{"image": "https://x/i.png"}"#);
        let p = parse_metadata(&event);
        assert_eq!(p.picture, "https://x/i.png");
    }

    #[test]
    fn unparseable_content_yields_pubkey_only_record() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, "not json");
        let p = parse_metadata(&event);
        assert_eq!(p.pubkey, keys.public_key().to_hex());
        assert!(p.name.is_empty());
        assert!(p.about.is_empty());
    }

    #[test]
    fn profile_display_projection_prefers_display_name() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey8,
            profile: Some(ProfileMetadata {
                pubkey: "abcdef123456".to_string(),
                name: "alice".to_string(),
                display_name: "Reader One".to_string(),
                about: String::new(),
                picture: "https://example.com/avatar.png".to_string(),
                banner: String::new(),
                nip05: String::new(),
                website: String::new(),
                lud16: String::new(),
                created_at: None,
            }),
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "Reader One".to_string(),
                display_initial: "R".to_string(),
                picture_url: "https://example.com/avatar.png".to_string(),
            }
        );
    }

    #[test]
    fn profile_display_projection_falls_back_to_name() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey8,
            profile: Some(ProfileMetadata {
                pubkey: "abcdef123456".to_string(),
                name: "alice".to_string(),
                display_name: String::new(),
                about: String::new(),
                picture: String::new(),
                banner: String::new(),
                nip05: String::new(),
                website: String::new(),
                lud16: String::new(),
                created_at: None,
            }),
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "alice".to_string(),
                display_initial: "a".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_projection_falls_back_to_pubkey() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey8,
            profile: None,
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "abcdef12".to_string(),
                display_initial: "a".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_projection_supports_profile_page_pubkey_fallback() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef1234567890".to_string(),
            fallback: ProfileDisplayFallback::Pubkey12,
            profile: None,
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "abcdef123456".to_string(),
                display_initial: "a".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_projection_supports_row_pubkey_fallback() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef1234567890".to_string(),
            fallback: ProfileDisplayFallback::Pubkey10,
            profile: None,
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "abcdef1234".to_string(),
                display_initial: "a".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_projection_supports_account_label_fallback() {
        let projection = profile_display_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::AccountLabel,
            profile: None,
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "Nostr Account".to_string(),
                display_initial: String::new(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_handle_projection_prefers_name_before_display_name() {
        let projection = profile_handle_projection(ProfileDisplayProjectionInput {
            pubkey: "fbcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey6,
            profile: Some(ProfileMetadata {
                pubkey: "fbcdef123456".to_string(),
                name: "alice".to_string(),
                display_name: "Reader One".to_string(),
                about: String::new(),
                picture: "https://example.com/avatar.png".to_string(),
                banner: String::new(),
                nip05: String::new(),
                website: String::new(),
                lud16: String::new(),
                created_at: None,
            }),
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "alice".to_string(),
                display_initial: "f".to_string(),
                picture_url: "https://example.com/avatar.png".to_string(),
            }
        );
    }

    #[test]
    fn profile_handle_projection_supports_pubkey6_fallback() {
        let projection = profile_handle_projection(ProfileDisplayProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey6,
            profile: None,
        });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "abcdef".to_string(),
                display_initial: "a".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_with_label_projection_prefers_label_before_pubkey() {
        let projection =
            profile_display_with_label_projection(ProfileDisplayWithLabelProjectionInput {
                pubkey: "abcdef1234567890".to_string(),
                profile: None,
                label_fallback: "Jane Author".to_string(),
                pubkey_fallback: ProfileDisplayFallback::Pubkey10,
                empty_fallback: "Unknown".to_string(),
            });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "Jane Author".to_string(),
                display_initial: "J".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_display_with_label_projection_keeps_profile_picture_with_label() {
        let projection =
            profile_display_with_label_projection(ProfileDisplayWithLabelProjectionInput {
                pubkey: "abcdef1234567890".to_string(),
                profile: Some(ProfileMetadata {
                    pubkey: "abcdef1234567890".to_string(),
                    name: String::new(),
                    display_name: String::new(),
                    about: String::new(),
                    picture: "https://example.com/p.png".to_string(),
                    banner: String::new(),
                    nip05: String::new(),
                    website: String::new(),
                    lud16: String::new(),
                    created_at: None,
                }),
                label_fallback: "Jane Author".to_string(),
                pubkey_fallback: ProfileDisplayFallback::Pubkey10,
                empty_fallback: "Unknown".to_string(),
            });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "Jane Author".to_string(),
                display_initial: "J".to_string(),
                picture_url: "https://example.com/p.png".to_string(),
            }
        );
    }

    #[test]
    fn profile_display_with_label_projection_uses_empty_fallback_without_label_or_pubkey() {
        let projection =
            profile_display_with_label_projection(ProfileDisplayWithLabelProjectionInput {
                pubkey: String::new(),
                profile: None,
                label_fallback: String::new(),
                pubkey_fallback: ProfileDisplayFallback::Pubkey10,
                empty_fallback: "Unknown".to_string(),
            });

        assert_eq!(
            projection,
            ProfileDisplayProjection {
                display_name: "Unknown".to_string(),
                display_initial: "U".to_string(),
                picture_url: String::new(),
            }
        );
    }

    #[test]
    fn profile_identity_projection_normalizes_root_nip05_label() {
        let projection = profile_identity_projection(ProfileIdentityProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey12,
            profile: Some(ProfileMetadata {
                pubkey: "abcdef123456".to_string(),
                name: "alice".to_string(),
                display_name: "Alice".to_string(),
                about: "Reader and writer".to_string(),
                picture: "https://example.com/avatar.png".to_string(),
                banner: String::new(),
                nip05: "_@example.com".to_string(),
                website: String::new(),
                lud16: String::new(),
                created_at: None,
            }),
        });

        assert_eq!(
            projection,
            ProfileIdentityProjection {
                display_name: "Alice".to_string(),
                display_initial: "A".to_string(),
                picture_url: "https://example.com/avatar.png".to_string(),
                bio: "Reader and writer".to_string(),
                verified_nip05: Some("example.com".to_string()),
            }
        );
    }

    #[test]
    fn profile_identity_projection_omits_empty_nip05() {
        let projection = profile_identity_projection(ProfileIdentityProjectionInput {
            pubkey: "abcdef123456".to_string(),
            fallback: ProfileDisplayFallback::Pubkey12,
            profile: None,
        });

        assert_eq!(projection.verified_nip05, None);
        assert!(projection.bio.is_empty());
    }

    #[test]
    fn profile_relationship_projection_detects_own_profile_case_insensitive() {
        let projection = profile_relationship_projection(ProfileRelationshipProjectionInput {
            profile_pubkey: "  ABCDEF  ".into(),
            viewer_pubkey: Some("abcdef".into()),
        });

        assert_eq!(projection.target_pubkey, "ABCDEF");
        assert!(projection.is_own_profile);
        assert!(!projection.can_show_follow_action);
        assert!(!projection.should_refresh_follow_state);
    }

    #[test]
    fn profile_relationship_projection_requires_logged_in_distinct_viewer() {
        let logged_out = profile_relationship_projection(ProfileRelationshipProjectionInput {
            profile_pubkey: "abcdef".into(),
            viewer_pubkey: None,
        });
        let distinct = profile_relationship_projection(ProfileRelationshipProjectionInput {
            profile_pubkey: "abcdef".into(),
            viewer_pubkey: Some("123456".into()),
        });

        assert!(!logged_out.is_own_profile);
        assert!(!logged_out.can_show_follow_action);
        assert!(!logged_out.should_refresh_follow_state);
        assert!(distinct.can_show_follow_action);
        assert!(distinct.should_refresh_follow_state);
    }

    #[test]
    fn profile_update_projection_trims_draft_and_preserves_raw_dirty_policy() {
        let projection = profile_update_projection(ProfileUpdateProjectionInput {
            initial: Some(ProfileMetadata {
                pubkey: "abcdef123456".to_string(),
                name: "alice".to_string(),
                display_name: "Alice".to_string(),
                about: "Bio".to_string(),
                picture: "https://example.com/p.png".to_string(),
                banner: "https://example.com/b.png".to_string(),
                nip05: "alice@example.com".to_string(),
                website: "https://example.com".to_string(),
                lud16: "alice@getalby.com".to_string(),
                created_at: None,
            }),
            name: " alice ".to_string(),
            display_name: " Alice ".to_string(),
            about: " Bio ".to_string(),
            picture: " https://example.com/p.png ".to_string(),
            banner: " https://example.com/b.png ".to_string(),
            nip05: " alice@example.com ".to_string(),
            website: " https://example.com ".to_string(),
            lud16: " alice@getalby.com ".to_string(),
            saving: false,
            picture_uploading: false,
            banner_uploading: false,
        });

        assert!(projection.is_dirty);
        assert!(projection.can_save);
        assert_eq!(projection.draft.name, "alice");
        assert_eq!(projection.draft.display_name, "Alice");
        assert_eq!(projection.draft.about, "Bio");
        assert_eq!(projection.draft.picture, "https://example.com/p.png");
        assert_eq!(projection.draft.banner, "https://example.com/b.png");
        assert_eq!(projection.draft.nip05, "alice@example.com");
        assert_eq!(projection.draft.website, "https://example.com");
        assert_eq!(projection.draft.lud16, "alice@getalby.com");
    }

    #[test]
    fn profile_update_projection_blocks_clean_or_busy_form() {
        let clean = profile_update_projection(ProfileUpdateProjectionInput {
            initial: None,
            name: String::new(),
            display_name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            saving: false,
            picture_uploading: false,
            banner_uploading: false,
        });
        let busy = profile_update_projection(ProfileUpdateProjectionInput {
            initial: None,
            name: "alice".to_string(),
            display_name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            saving: true,
            picture_uploading: false,
            banner_uploading: false,
        });
        let uploading = profile_update_projection(ProfileUpdateProjectionInput {
            initial: None,
            name: "alice".to_string(),
            display_name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            saving: false,
            picture_uploading: true,
            banner_uploading: false,
        });

        assert!(!clean.is_dirty);
        assert!(!clean.can_save);
        assert!(busy.is_dirty);
        assert!(!busy.can_save);
        assert!(uploading.is_dirty);
        assert!(!uploading.can_save);
    }
}
