//! FFI surface for the new nmp-lane kernel.
//!
//! `ios.rs` exports `HighlighterApp` via UniFFI. The `HighlighterCore`
//! object in `client.rs` is untouched — the two objects coexist in the
//! same UniFFI scaffolding.

pub mod ios;

pub use ios::HighlighterApp;
