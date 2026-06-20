//! Audio capability types — Phase 5H.
//!
//! The Rust kernel decides *what* the native audio player should do; native
//! (iOS `AVPlayer` / Android `ExoPlayer`) executes the raw platform transport
//! and reports raw progress back at a **bounded cadence** (D8: never 0.25 s
//! per-frame FFI traffic — native coalesces to ≤1 s before crossing FFI or
//! the kernel down-samples via the injected clock).
//!
//! Rust owns all playback STATE, resume policy, seek bounds, and clip
//! semantics. Native is a capability executor only (D7).

/// What the kernel is asking the native audio player to do.
///
/// Append-only: new ops extend the transport surface without breaking
/// existing native handlers.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum AudioOp {
    /// Load and optionally seek the player to `resume_at_seconds` before
    /// starting.  When `resume_at_seconds` is `None` the player starts from
    /// the beginning. After loading the player must report back a
    /// `AudioResult::Loaded { duration_seconds }` result.
    Load {
        /// HTTP(S) audio URL. Opaque from the kernel (D3: no URL construction).
        url: String,
        /// Optional resume position in seconds (≥ 0, finite).  `None` → start
        /// from beginning.
        resume_at_seconds: Option<f64>,
    },
    /// Begin / resume playback of the already-loaded item.
    Play,
    /// Pause playback without unloading the item.
    Pause,
    /// Seek to `seconds` (clamped to `[0, duration]` by the kernel before
    /// this op is emitted, but native should also clamp defensively).
    Seek {
        /// Target position in seconds (finite, ≥ 0).
        seconds: f64,
    },
    /// Stop playback and unload the item.  A new `Load` is required before
    /// playing again.
    Stop,
    /// Extract waveform peaks from the audio at `url` (may be different from
    /// the currently loaded item — e.g. pre-fetching a clip).  Native decodes
    /// the audio and returns `bucket_count` normalized amplitude buckets in
    /// `AudioResult::WaveformPeaks`.  Should run off the main thread.
    ExtractWaveform {
        /// Audio URL to extract from (opaque from kernel).
        url: String,
        /// Number of amplitude buckets to return (typically 100–200).
        bucket_count: u32,
    },
}

/// Raw result from the native audio player, reported via
/// `provide_capability_result`. Errors are data (D6) — never panics.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum AudioResult {
    /// Periodic progress update.  Native reports at most once per ~1 second
    /// (bounded cadence, D8 — not 0.25 s raw `addPeriodicTimeObserver` ticks).
    /// The kernel coalesces via the injected clock if native sends more often.
    Progress {
        /// Current playback position in seconds (finite, ≥ 0).
        current_seconds: f64,
        /// `true` when the player is actively playing.
        is_playing: bool,
    },
    /// The `Load` op completed successfully.  Sent once after item loads.
    Loaded {
        /// Total duration of the loaded item in seconds (> 0).
        duration_seconds: f64,
    },
    /// Waveform extraction finished.  One bucket per normalized amplitude
    /// value in `[0.0, 1.0]`. Empty if extraction failed (D6).
    WaveformPeaks {
        /// URL this waveform corresponds to (echoed from the `ExtractWaveform`
        /// op so the kernel can key the cache by URL).
        url: String,
        /// Normalized amplitude buckets in `[0.0, 1.0]`.
        buckets: Vec<f32>,
    },
    /// Playback reached the end of the item naturally.
    Ended,
    /// A transport or loading error occurred.  The kernel surfaces this as
    /// typed state (D6 — never a `Result`).
    Error(String),
}
