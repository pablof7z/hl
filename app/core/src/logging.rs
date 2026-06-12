//! Platform logging bootstrap. The core logs through `tracing`; without a
//! subscriber every event is dropped, so each host must call
//! [`init_platform_logging`] once at startup (before `Bootstrap`).
//!
//! - Android: forwards to logcat under the `highlighter-core` tag.
//! - iOS / desktop / tests: line-formatted output on stderr (Xcode console).
//!
//! Level defaults to `info` and can be raised with the standard
//! `RUST_LOG`-style directive via [`init_platform_logging_with_filter`].

use std::sync::Once;

static INIT: Once = Once::new();

/// Install the platform tracing subscriber at `info` level. Idempotent.
#[uniffi::export]
pub fn init_platform_logging() {
    init_with(None);
}

/// Install the platform tracing subscriber with an explicit filter directive
/// (e.g. `"debug"` or `"highlighter_core=trace,nmp_core=debug"`). Idempotent —
/// the first call wins.
#[uniffi::export]
pub fn init_platform_logging_with_filter(filter: String) {
    init_with(Some(filter));
}

fn init_with(filter: Option<String>) {
    INIT.call_once(|| {
        let directive = filter.unwrap_or_else(|| "info".to_string());

        #[cfg(target_os = "android")]
        {
            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;
            match tracing_android::layer("highlighter-core") {
                Ok(layer) => {
                    let _ = tracing_subscriber::registry()
                        .with(tracing_subscriber::EnvFilter::new(directive))
                        .with(layer)
                        .try_init();
                }
                Err(_) => {
                    // Fall through silently: logging must never crash the host.
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new(directive))
                .with_writer(std::io::stderr)
                .try_init();
        }
    });
}
