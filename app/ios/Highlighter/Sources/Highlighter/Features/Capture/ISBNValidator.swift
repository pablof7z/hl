import Foundation

/// Thin Swift shim over Rust-owned ISBN normalization. Native scanner/manual
/// entry code reports raw text; the core owns Bookland prefix, checksum, and
/// ISBN-10 conversion semantics.
enum ISBNValidator {
    static func validate(_ raw: String) -> String? {
        try? normalizeIsbn(raw: raw)
    }
}
