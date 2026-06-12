import Foundation

/// Typed navigation destination for a user profile. Kept in its own enum so
/// we can later add `.npub(String)` / `.nip05(String)` cases without touching
/// every call site.
enum ProfileDestination: Hashable, Identifiable {
    case pubkey(String)

    /// Stable identity for `.navigationDestination(item:)` / `.sheet(item:)`.
    var id: String {
        switch self {
        case .pubkey(let pk): return "pubkey:\(pk)"
        }
    }
}
