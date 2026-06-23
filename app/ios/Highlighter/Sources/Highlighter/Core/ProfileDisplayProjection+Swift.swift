import Foundation

extension ProfileDisplayProjection {
    static func from(pubkey: String, profile: ProfileMetadata?) -> ProfileDisplayProjection {
        let rawName = profile.map { !$0.displayName.isEmpty ? $0.displayName : $0.name } ?? ""
        let name = rawName.isEmpty ? String(pubkey.prefix(10)) : rawName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
            pictureUrl: profile?.picture ?? ""
        )
    }
}
