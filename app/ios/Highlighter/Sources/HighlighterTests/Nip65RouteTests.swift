import Foundation
import Testing
@testable import Highlighter

/// Regression guard for the kind:10002 (NIP-65) write routing in
/// `NetworkSettingsStore`. The marker decision itself is kernel-owned
/// (`nip65RelayRole`) and parity-tested against bespoke `nip65_tags` in Rust;
/// this pins the Swift-side routing that consumes it — most importantly the
/// bug site that shipped in 7e2de4f3 and was fixed in b62324e5: a
/// rooms/indexer-only relay (read=write=false) MUST be removed from kind:10002,
/// never added with a "both" marker.
struct Nip65RouteTests {

    @Test func bothFlagsSetRoleBoth() {
        #expect(NetworkSettingsStore.nip65Route(read: true, write: true) == .setRole("both"))
    }

    @Test func readOnlySetRoleRead() {
        #expect(NetworkSettingsStore.nip65Route(read: true, write: false) == .setRole("read"))
    }

    @Test func writeOnlySetRoleWrite() {
        #expect(NetworkSettingsStore.nip65Route(read: false, write: true) == .setRole("write"))
    }

    /// The regression site: a relay with neither read nor write must be REMOVED
    /// from kind:10002 (it lives only in the kind:30078 app-data), not added.
    @Test func neitherFlagRemoves() {
        #expect(NetworkSettingsStore.nip65Route(read: false, write: false) == .remove)
    }
}
