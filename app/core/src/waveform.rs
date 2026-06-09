//! Podcast waveform extraction policy.
//!
//! Native shells execute platform audio, filesystem, and network capabilities.
//! The core owns cache identity, extraction gating, and bucket sizing so every
//! platform follows the same policy.

use sha2::{Digest, Sha256};

const MIN_WAVEFORM_BUCKETS: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WaveformWifiStatus {
    Unknown,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WaveformCacheKeyProjectionInput {
    pub audio_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WaveformCacheKeyProjection {
    pub cache_key: String,
    pub is_usable: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WaveformPeaksPlanInput {
    pub audio_url: String,
    pub duration_seconds: f64,
    pub cached_peaks_available: bool,
    pub wifi_status: WaveformWifiStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WaveformPeaksPlan {
    pub cache_key: String,
    pub should_use_cached_peaks: bool,
    pub should_check_wifi_status: bool,
    pub should_extract_peaks: bool,
    pub bucket_count: u32,
    pub skip_reason: String,
}

pub fn cache_key_projection(input: WaveformCacheKeyProjectionInput) -> WaveformCacheKeyProjection {
    match cache_key_for_audio_url(&input.audio_url) {
        Some(cache_key) => WaveformCacheKeyProjection {
            cache_key,
            is_usable: true,
        },
        None => WaveformCacheKeyProjection {
            cache_key: String::new(),
            is_usable: false,
        },
    }
}

pub fn peaks_plan(input: WaveformPeaksPlanInput) -> WaveformPeaksPlan {
    let Some(cache_key) = cache_key_for_audio_url(&input.audio_url) else {
        return WaveformPeaksPlan {
            cache_key: String::new(),
            should_use_cached_peaks: false,
            should_check_wifi_status: false,
            should_extract_peaks: false,
            bucket_count: 0,
            skip_reason: "missing_audio_url".to_string(),
        };
    };

    let bucket_count = waveform_bucket_count(input.duration_seconds);
    if input.cached_peaks_available {
        return WaveformPeaksPlan {
            cache_key,
            should_use_cached_peaks: true,
            should_check_wifi_status: false,
            should_extract_peaks: false,
            bucket_count,
            skip_reason: String::new(),
        };
    }

    match input.wifi_status {
        WaveformWifiStatus::Unknown => WaveformPeaksPlan {
            cache_key,
            should_use_cached_peaks: false,
            should_check_wifi_status: true,
            should_extract_peaks: false,
            bucket_count,
            skip_reason: "wifi_status_required".to_string(),
        },
        WaveformWifiStatus::Available => WaveformPeaksPlan {
            cache_key,
            should_use_cached_peaks: false,
            should_check_wifi_status: false,
            should_extract_peaks: true,
            bucket_count,
            skip_reason: String::new(),
        },
        WaveformWifiStatus::Unavailable => WaveformPeaksPlan {
            cache_key,
            should_use_cached_peaks: false,
            should_check_wifi_status: false,
            should_extract_peaks: false,
            bucket_count,
            skip_reason: "wifi_required".to_string(),
        },
    }
}

fn cache_key_for_audio_url(audio_url: &str) -> Option<String> {
    let trimmed = audio_url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    Some(hex::encode(hasher.finalize()))
}

fn waveform_bucket_count(duration_seconds: f64) -> u32 {
    let rounded_seconds = if duration_seconds.is_finite() && duration_seconds > 0.0 {
        duration_seconds.round()
    } else {
        0.0
    };
    rounded_seconds
        .max(MIN_WAVEFORM_BUCKETS as f64)
        .min(u32::MAX as f64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUDIO_URL: &str = "https://example.com/audio.mp3";
    const AUDIO_URL_SHA256: &str =
        "a0c4f2ad3757186089a63bfb6876001b56e659352b5b74a896b63f2de537300f";

    #[test]
    fn cache_key_hashes_trimmed_audio_url() {
        let projection = cache_key_projection(WaveformCacheKeyProjectionInput {
            audio_url: format!("  {AUDIO_URL}  "),
        });

        assert!(projection.is_usable);
        assert_eq!(projection.cache_key, AUDIO_URL_SHA256);
    }

    #[test]
    fn empty_audio_url_is_not_usable() {
        let projection = cache_key_projection(WaveformCacheKeyProjectionInput {
            audio_url: "   ".to_string(),
        });
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: "   ".to_string(),
            duration_seconds: 120.0,
            cached_peaks_available: false,
            wifi_status: WaveformWifiStatus::Available,
        });

        assert!(!projection.is_usable);
        assert_eq!(projection.cache_key, "");
        assert_eq!(plan.skip_reason, "missing_audio_url");
        assert!(!plan.should_extract_peaks);
    }

    #[test]
    fn cached_peaks_win_without_network_status() {
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: AUDIO_URL.to_string(),
            duration_seconds: 180.0,
            cached_peaks_available: true,
            wifi_status: WaveformWifiStatus::Unknown,
        });

        assert_eq!(plan.cache_key, AUDIO_URL_SHA256);
        assert!(plan.should_use_cached_peaks);
        assert!(!plan.should_check_wifi_status);
        assert!(!plan.should_extract_peaks);
        assert_eq!(plan.bucket_count, 180);
    }

    #[test]
    fn missing_cache_requests_wifi_status() {
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: AUDIO_URL.to_string(),
            duration_seconds: 42.0,
            cached_peaks_available: false,
            wifi_status: WaveformWifiStatus::Unknown,
        });

        assert!(plan.should_check_wifi_status);
        assert!(!plan.should_extract_peaks);
        assert_eq!(plan.bucket_count, MIN_WAVEFORM_BUCKETS);
    }

    #[test]
    fn missing_wifi_skips_extraction() {
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: AUDIO_URL.to_string(),
            duration_seconds: 120.0,
            cached_peaks_available: false,
            wifi_status: WaveformWifiStatus::Unavailable,
        });

        assert!(!plan.should_use_cached_peaks);
        assert!(!plan.should_check_wifi_status);
        assert!(!plan.should_extract_peaks);
        assert_eq!(plan.skip_reason, "wifi_required");
    }

    #[test]
    fn available_wifi_extracts_with_bounded_bucket_count() {
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: AUDIO_URL.to_string(),
            duration_seconds: 3_600.4,
            cached_peaks_available: false,
            wifi_status: WaveformWifiStatus::Available,
        });

        assert!(plan.should_extract_peaks);
        assert_eq!(plan.bucket_count, 3_600);
        assert_eq!(plan.skip_reason, "");
    }

    #[test]
    fn invalid_duration_uses_minimum_bucket_count() {
        let plan = peaks_plan(WaveformPeaksPlanInput {
            audio_url: AUDIO_URL.to_string(),
            duration_seconds: f64::NAN,
            cached_peaks_available: false,
            wifi_status: WaveformWifiStatus::Available,
        });

        assert!(plan.should_extract_peaks);
        assert_eq!(plan.bucket_count, MIN_WAVEFORM_BUCKETS);
    }
}
