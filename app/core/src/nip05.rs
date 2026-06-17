//! Highlighter-managed NIP-05 username projections.
//!
//! The native shell owns text fields and rendering. The Rust core owns the
//! username rules and onboarding projection shape. Account creation no longer
//! registers a managed username during onboarding.

use crate::errors::CoreError;

#[derive(Debug, Clone, uniffi::Record)]
pub struct Nip05Availability {
    pub valid: bool,
    pub available: bool,
    pub identifier: String,
    pub domain: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Nip05AvailabilityState {
    Idle,
    Invalid,
    Available,
    Taken,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Nip05AvailabilitySnapshot {
    pub state: Nip05AvailabilityState,
    pub identifier: String,
    pub domain: String,
    pub error_message: String,
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OnboardingUsernameCheckProjection {
    pub username: String,
    pub has_username: bool,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Nip05RegistrationSnapshot {
    pub identifier: Option<String>,
    pub succeeded: bool,
    pub error_message: Option<String>,
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

pub fn onboarding_username_check_projection(input: &str) -> OnboardingUsernameCheckProjection {
    let username = input.trim().to_string();
    let has_username = !username.is_empty();
    let valid = has_username && is_valid_username(&username);
    OnboardingUsernameCheckProjection {
        username,
        has_username,
        valid,
    }
}

pub fn registration_snapshot(result: Result<String, CoreError>) -> Nip05RegistrationSnapshot {
    match result {
        Ok(identifier) => Nip05RegistrationSnapshot {
            identifier: Some(identifier),
            succeeded: true,
            error_message: None,
        },
        Err(error) => Nip05RegistrationSnapshot {
            identifier: None,
            succeeded: false,
            error_message: Some(error.to_string()),
        },
    }
}

pub fn availability_snapshot(
    result: Result<Nip05Availability, CoreError>,
) -> Nip05AvailabilitySnapshot {
    match result {
        Ok(availability) if !availability.valid => Nip05AvailabilitySnapshot {
            state: Nip05AvailabilityState::Invalid,
            identifier: String::new(),
            domain: String::new(),
            error_message: String::new(),
        },
        Ok(availability) if availability.available => Nip05AvailabilitySnapshot {
            state: Nip05AvailabilityState::Available,
            identifier: availability.identifier,
            domain: availability.domain,
            error_message: String::new(),
        },
        Ok(_) => Nip05AvailabilitySnapshot {
            state: Nip05AvailabilityState::Taken,
            identifier: String::new(),
            domain: String::new(),
            error_message: String::new(),
        },
        Err(error) => Nip05AvailabilitySnapshot {
            state: Nip05AvailabilityState::Idle,
            identifier: String::new(),
            domain: String::new(),
            error_message: error.to_string(),
        },
    }
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

    #[test]
    fn onboarding_username_check_projection_trims_and_validates() {
        let blank = onboarding_username_check_projection(" ");
        let valid = onboarding_username_check_projection(" alice ");
        let invalid = onboarding_username_check_projection(" Alice ");

        assert_eq!(blank.username, "");
        assert!(!blank.has_username);
        assert!(!blank.valid);
        assert_eq!(valid.username, "alice");
        assert!(valid.has_username);
        assert!(valid.valid);
        assert_eq!(invalid.username, "Alice");
        assert!(invalid.has_username);
        assert!(!invalid.valid);
    }

    #[test]
    fn registration_snapshot_projects_success_and_error_states() {
        let success = registration_snapshot(Ok("alice@highlighter.com".into()));
        assert_eq!(success.identifier.as_deref(), Some("alice@highlighter.com"));
        assert!(success.succeeded);
        assert_eq!(success.error_message, None);

        let failure = registration_snapshot(Err(CoreError::Network("taken".into())));
        assert_eq!(failure.identifier, None);
        assert!(!failure.succeeded);
        assert_eq!(
            failure.error_message.as_deref(),
            Some("network error: taken")
        );
    }

    #[test]
    fn availability_snapshot_projects_username_states_and_errors() {
        let invalid = availability_snapshot(Ok(Nip05Availability {
            valid: false,
            available: false,
            identifier: String::new(),
            domain: String::new(),
        }));
        assert_eq!(invalid.state, Nip05AvailabilityState::Invalid);
        assert!(invalid.identifier.is_empty());

        let available = availability_snapshot(Ok(Nip05Availability {
            valid: true,
            available: true,
            identifier: "alice@highlighter.com".into(),
            domain: "highlighter.com".into(),
        }));
        assert_eq!(available.state, Nip05AvailabilityState::Available);
        assert_eq!(available.identifier, "alice@highlighter.com");
        assert_eq!(available.domain, "highlighter.com");

        let taken = availability_snapshot(Ok(Nip05Availability {
            valid: true,
            available: false,
            identifier: "alice@highlighter.com".into(),
            domain: "highlighter.com".into(),
        }));
        assert_eq!(taken.state, Nip05AvailabilityState::Taken);
        assert!(taken.identifier.is_empty());

        let failure = availability_snapshot(Err(CoreError::Network("offline".into())));
        assert_eq!(failure.state, Nip05AvailabilityState::Idle);
        assert_eq!(failure.error_message, "network error: offline");
    }
}
