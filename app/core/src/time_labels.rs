#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RelativeTimeLabelStyle {
    Compact,
    Ago,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RelativeTimeLabelInput {
    pub unix_seconds: Option<u64>,
    pub style: RelativeTimeLabelStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelativeTimeLabelProjection {
    pub label: Option<String>,
}

/// Product relative-time badge projection. Rust owns the injected-clock delta
/// calculation; native shells render the returned string or omit the badge.
pub fn relative_time_label_projection(
    input: RelativeTimeLabelInput,
    now_unix_seconds: u64,
) -> RelativeTimeLabelProjection {
    let Some(unix_seconds) = input.unix_seconds.filter(|seconds| *seconds > 0) else {
        return RelativeTimeLabelProjection { label: None };
    };
    let Some(delta) = now_unix_seconds.checked_sub(unix_seconds) else {
        return RelativeTimeLabelProjection { label: None };
    };

    let base = match delta {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m", delta / 60),
        3_600..=86_399 => format!("{}h", delta / 3_600),
        86_400..=604_799 => format!("{}d", delta / 86_400),
        604_800..=2_591_999 => format!("{}w", delta / 604_800),
        _ => format!("{}mo", delta / 2_592_000),
    };

    let label = match input.style {
        RelativeTimeLabelStyle::Compact => base,
        RelativeTimeLabelStyle::Ago if base == "just now" => base,
        RelativeTimeLabelStyle::Ago => format!("{base} ago"),
    };

    RelativeTimeLabelProjection { label: Some(label) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(seconds: Option<u64>, now: u64, style: RelativeTimeLabelStyle) -> Option<String> {
        relative_time_label_projection(
            RelativeTimeLabelInput {
                unix_seconds: seconds,
                style,
            },
            now,
        )
        .label
    }

    #[test]
    fn compact_relative_time_matches_highlight_badges() {
        let now = 10_000_000;

        assert_eq!(
            project(Some(now - 30), now, RelativeTimeLabelStyle::Compact),
            Some("just now".into())
        );
        assert_eq!(
            project(Some(now - 120), now, RelativeTimeLabelStyle::Compact),
            Some("2m".into())
        );
        assert_eq!(
            project(Some(now - 7_200), now, RelativeTimeLabelStyle::Compact),
            Some("2h".into())
        );
        assert_eq!(
            project(Some(now - 172_800), now, RelativeTimeLabelStyle::Compact),
            Some("2d".into())
        );
        assert_eq!(
            project(Some(now - 1_209_600), now, RelativeTimeLabelStyle::Compact),
            Some("2w".into())
        );
        assert_eq!(
            project(Some(now - 5_184_000), now, RelativeTimeLabelStyle::Compact),
            Some("2mo".into())
        );
    }

    #[test]
    fn ago_relative_time_keeps_just_now_unsuffixed() {
        let now = 10_000_000;

        assert_eq!(
            project(Some(now - 30), now, RelativeTimeLabelStyle::Ago),
            Some("just now".into())
        );
        assert_eq!(
            project(Some(now - 120), now, RelativeTimeLabelStyle::Ago),
            Some("2m ago".into())
        );
    }

    #[test]
    fn invalid_or_future_timestamps_do_not_render() {
        assert_eq!(project(None, 1_000, RelativeTimeLabelStyle::Compact), None);
        assert_eq!(
            project(Some(0), 1_000, RelativeTimeLabelStyle::Compact),
            None
        );
        assert_eq!(
            project(Some(2_000), 1_000, RelativeTimeLabelStyle::Compact),
            None
        );
    }
}
