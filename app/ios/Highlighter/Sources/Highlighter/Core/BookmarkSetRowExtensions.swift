// BookmarkSetRowExtensions.swift
// Helpers on `BookmarkSetRow` (generated UniFFI type) that are too
// app-specific to live in the Rust layer.

import Foundation

extension BookmarkSetRow {
    /// The NIP-33 coordinate string for this set: `"\(kind):\(pubkey):\(dTag)"`.
    ///
    /// Used when dispatching `addToSet`, `removeFromSet`, `renameSet`, and
    /// `deleteSet` actions so the coordinate is computed from the canonical
    /// `(kind, pubkey, dTag)` triple rather than being inlined at each call site.
    ///
    /// Extracted from `BookmarkMenuButton.toggleInCuration` (was L113) per
    /// issue #63 to make it testable and reusable.
    public var setCoordinate: String {
        "\(kind):\(pubkey):\(dTag)"
    }
}
