//! Highlighter-managed NIP-05 username availability and registration.
//!
//! The native shell owns text fields and rendering. The Rust core owns the
//! username rules, service endpoint, signed auth event, HTTP request shape,
//! and server error interpretation.

use ::url::Url;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::CoreError;
use crate::nostr_runtime::NostrRuntime;

const NIP05_BASE_URL: &str = "https://beta.highlighter.com";
const KIND_NIP05_AUTH: u16 = 27235;

#[derive(Debug, Clone, uniffi::Record)]
pub struct Nip05Availability {
    pub valid: bool,
    pub available: bool,
    pub identifier: String,
    pub domain: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct OnboardingCreateAccountProjectionInput {
    pub display_name: String,
    pub username: String,
    pub username_available: bool,
    pub is_working: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OnboardingCreateAccountProjection {
    pub display_name: String,
    pub username: String,
    pub can_continue: bool,
}

#[derive(Debug, Deserialize)]
struct AvailabilityResponse {
    available: bool,
    identifier: String,
}

#[derive(Debug, Serialize)]
struct RegisterRequest {
    name: String,
    auth: Value,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

pub fn normalize_username(input: &str) -> String {
    input.to_lowercase()
}

pub fn suggest_username(display_name: &str) -> String {
    let mut out = String::with_capacity(display_name.len());
    let mut previous_was_separator = false;

    for ch in display_name.trim().to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_' {
            out.push(ch);
            previous_was_separator = false;
        } else if ch.is_whitespace() && !out.is_empty() && !previous_was_separator {
            out.push('_');
            previous_was_separator = true;
        }
    }

    while out.ends_with('_') {
        out.pop();
    }
    out
}

pub fn is_valid_username(input: &str) -> bool {
    let trimmed = input.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn onboarding_create_account_projection(
    input: OnboardingCreateAccountProjectionInput,
) -> OnboardingCreateAccountProjection {
    let display_name = input.display_name.trim().to_string();
    let username = input.username.trim().to_string();
    let username_allows_continue = username.is_empty() || input.username_available;
    OnboardingCreateAccountProjection {
        can_continue: !input.is_working && !display_name.is_empty() && username_allows_continue,
        display_name,
        username,
    }
}

pub async fn check_availability(name: &str) -> Result<Nip05Availability, CoreError> {
    let name = name.trim();
    if !is_valid_username(name) {
        return Ok(Nip05Availability {
            valid: false,
            available: false,
            identifier: String::new(),
            domain: String::new(),
        });
    }

    let mut url = Url::parse(NIP05_BASE_URL)
        .map_err(|e| CoreError::InvalidInput(format!("invalid nip05 base URL: {e}")))?;
    url.set_path("api/nip05");
    url.query_pairs_mut().append_pair("name", name);

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("nip05 availability: {e}")))?;

    let decoded = response
        .json::<AvailabilityResponse>()
        .await
        .map_err(|e| CoreError::Network(format!("nip05 availability response: {e}")))?;

    let domain = decoded
        .identifier
        .rsplit_once('@')
        .map(|(_, domain)| domain.to_string())
        .unwrap_or_else(|| "highlighter.com".to_string());

    Ok(Nip05Availability {
        valid: true,
        available: decoded.available,
        identifier: decoded.identifier,
        domain,
    })
}

pub async fn register_username(
    runtime: &NostrRuntime,
    name: &str,
    domain: &str,
) -> Result<String, CoreError> {
    let name = name.trim();
    let domain = domain.trim();
    if !is_valid_username(name) {
        return Err(CoreError::InvalidInput("invalid NIP-05 username".into()));
    }
    if domain.is_empty() {
        return Err(CoreError::InvalidInput("NIP-05 domain is required".into()));
    }

    let auth_json = sign_registration_auth(runtime, name, domain).await?;
    let auth = serde_json::from_str::<Value>(&auth_json)
        .map_err(|e| CoreError::Other(format!("parse NIP-05 auth event: {e}")))?;
    let body = RegisterRequest {
        name: name.to_string(),
        auth,
    };

    let mut url = Url::parse(NIP05_BASE_URL)
        .map_err(|e| CoreError::InvalidInput(format!("invalid nip05 base URL: {e}")))?;
    url.set_path("api/nip05");

    let response = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("nip05 registration: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        let bytes = response.bytes().await.unwrap_or_default();
        let message = serde_json::from_slice::<ErrorResponse>(&bytes)
            .map(|err| err.error)
            .unwrap_or_else(|_| format!("Registration failed ({})", status.as_u16()));
        return Err(CoreError::Network(message));
    }

    Ok(format!("{name}@{domain}"))
}

async fn sign_registration_auth(
    runtime: &NostrRuntime,
    name: &str,
    domain: &str,
) -> Result<String, CoreError> {
    let tags = vec![
        parse_tag(&["t", "nip05-registration"])?,
        parse_tag(&["action", "register"])?,
        parse_tag(&["domain", domain])?,
        parse_tag(&["name", name])?,
    ];

    let builder = EventBuilder::new(Kind::Custom(KIND_NIP05_AUTH), "").tags(tags);
    let event = runtime
        .client()
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign nip05 registration auth: {e}")))?;
    Ok(event.as_json())
}

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_validation_matches_service_rules() {
        assert!(is_valid_username("alice"));
        assert!(is_valid_username("alice_1-test"));
        assert!(!is_valid_username(""));
        assert!(!is_valid_username("Alice"));
        assert!(!is_valid_username("alice space"));
        assert!(!is_valid_username("álice"));
        assert!(!is_valid_username(&"a".repeat(65)));
    }

    #[test]
    fn suggest_username_is_ascii_and_stable() {
        assert_eq!(suggest_username("Alice Smith"), "alice_smith");
        assert_eq!(suggest_username("  Alice   Smith  "), "alice_smith");
        assert_eq!(suggest_username("Alice 🎧 Smith"), "alice_smith");
        assert_eq!(suggest_username("Élodie"), "lodie");
    }

    #[test]
    fn normalize_username_lowercases_without_trimming_the_field() {
        assert_eq!(normalize_username(" Alice "), " alice ");
    }

    #[test]
    fn onboarding_create_account_projection_trims_and_gates_continue() {
        let no_username =
            onboarding_create_account_projection(OnboardingCreateAccountProjectionInput {
                display_name: " Alice ".into(),
                username: " ".into(),
                username_available: false,
                is_working: false,
            });
        let available =
            onboarding_create_account_projection(OnboardingCreateAccountProjectionInput {
                display_name: " Alice ".into(),
                username: " alice ".into(),
                username_available: true,
                is_working: false,
            });
        let unavailable =
            onboarding_create_account_projection(OnboardingCreateAccountProjectionInput {
                display_name: " Alice ".into(),
                username: " alice ".into(),
                username_available: false,
                is_working: false,
            });
        let working =
            onboarding_create_account_projection(OnboardingCreateAccountProjectionInput {
                display_name: " Alice ".into(),
                username: String::new(),
                username_available: false,
                is_working: true,
            });

        assert_eq!(no_username.display_name, "Alice");
        assert_eq!(available.username, "alice");
        assert!(no_username.can_continue);
        assert!(available.can_continue);
        assert!(!unavailable.can_continue);
        assert!(!working.can_continue);
    }
}
