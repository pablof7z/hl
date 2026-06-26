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

extension BookmarkSetRecord {
    /// The NIP-33 coordinate string for this set: `"\(kind):\(pubkey):\(id)"`.
    ///
    /// `BookmarkSetRecord.id` is the `d` tag (see generated doc comment), so the
    /// coordinate is the same `<kind>:<pubkey>:<d_tag>` shape used by
    /// `BookmarkSetRow.setCoordinate`. Used by the collections UI
    /// (`BookmarksView` / `SetDetailView`) for rename/delete/share dispatch.
    public var setCoordinate: String {
        "\(kind):\(pubkey):\(id)"
    }

    /// True when this set is authored by `pubkeyHex` (the active account).
    /// Edit affordances (rename/delete) are gated on this — following/curation
    /// sets owned by others are read-only / share-only (issue #63).
    public func isOwned(by pubkeyHex: String?) -> Bool {
        guard let pubkeyHex, !pubkeyHex.isEmpty else { return false }
        return pubkey == pubkeyHex
    }
}
