import Foundation

/// Typed navigation destination for a user profile. Rust resolves identity
/// inputs before Swift receives this route.
enum ProfileDestination: Hashable {
    case pubkey(String)
}
