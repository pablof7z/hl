#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RelativeTimeLabelStyle {
    Compact,
    Ago,
    BookmarkCompact,
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

    let base = match input.style {
        RelativeTimeLabelStyle::BookmarkCompact => compact_label(delta, false),
        RelativeTimeLabelStyle::Compact | RelativeTimeLabelStyle::Ago => compact_label(delta, true),
    };

    let label = match input.style {
        RelativeTimeLabelStyle::Compact | RelativeTimeLabelStyle::BookmarkCompact => base,
        RelativeTimeLabelStyle::Ago if base == "just now" => base,
        RelativeTimeLabelStyle::Ago => format!("{base} ago"),
    };

    RelativeTimeLabelProjection { label: Some(label) }
}

fn compact_label(delta_seconds: u64, just_now_under_minute: bool) -> String {
    match delta_seconds {
        0..=59 if just_now_under_minute => "just now".to_string(),
        0..=3_599 => format!("{}m", delta_seconds / 60),
        3_600..=86_399 => format!("{}h", delta_seconds / 3_600),
        86_400..=604_799 => format!("{}d", delta_seconds / 86_400),
        604_800..=2_591_999 => format!("{}w", delta_seconds / 604_800),
        _ => format!("{}mo", delta_seconds / 2_592_000),
    }
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
    fn bookmark_compact_relative_time_preserves_zero_minute_badge() {
        let now = 10_000_000;

        assert_eq!(
            project(Some(now - 30), now, RelativeTimeLabelStyle::BookmarkCompact),
            Some("0m".into())
        );
        assert_eq!(
            project(
                Some(now - 3_599),
                now,
                RelativeTimeLabelStyle::BookmarkCompact
            ),
            Some("59m".into())
        );
        assert_eq!(
            project(
                Some(now - 3_600),
                now,
                RelativeTimeLabelStyle::BookmarkCompact
            ),
            Some("1h".into())
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
