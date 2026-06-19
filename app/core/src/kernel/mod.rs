//! Rust kernel — the TEA (The Elm Architecture) heart of the nmp-lane.
//!
//! The kernel owns `AppState`, all reducer logic, the effect runner, the
//! view registry, and the coalesced observer. Native shells communicate via
//! the bounded FFI surface in `crate::ffi`.

pub mod action;
pub mod actor;
pub mod app;
pub mod clock;
pub mod effect;
pub mod snapshot;
pub mod view;

pub use action::{AppAction, KernelEvent, RootTab};
pub use actor::HighlighterObserver;
pub use app::{AppConfig, AppState};
pub use clock::{Clock, ManualClock, SystemClock};
pub use snapshot::{AppRootSnapshot, RootShellSnapshot, RouteKind, ToastSnapshot, ViewSnapshot};
pub use view::{ViewId, ViewRegistry, ViewRoute};
