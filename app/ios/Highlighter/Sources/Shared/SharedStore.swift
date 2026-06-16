import Foundation

/// App Group glue shared between the main app and the Share Extension.
/// Both targets compile this file; they talk to each other through small
/// JSON files in the App Group container and nothing else.
///
/// Design: the extension process is intentionally tiny. It does *not* load
/// the Rust core, touch the Keychain, or talk to any relay. It writes a
/// `PendingShare` into `ShareQueue` and opens the main app via the custom
/// URL scheme; the main app drains the queue on next foreground and
/// publishes via the Rust core using whichever signer is installed.
public enum AppGroup {
    public static let id = "group.com.highlighter.app"

    public static var containerURL: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: id)
    }

    static func fileURL(_ fileName: String) -> URL? {
        containerURL?.appendingPathComponent(fileName, isDirectory: false)
    }
}

private enum AppGroupJSONFiles {
    static func load<T: Decodable>(_ type: T.Type, fileName: String) -> T? {
        guard let url = AppGroup.fileURL(fileName),
              let data = FileManager.default.contents(atPath: url.path) else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }

    static func save<T: Encodable>(_ value: T, fileName: String) {
        guard let data = try? JSONEncoder().encode(value) else { return }
        saveData(data, fileName: fileName)
    }

    static func saveData(_ data: Data, fileName: String) {
        guard let url = AppGroup.fileURL(fileName) else { return }
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try data.write(to: url, options: [.atomic])
        } catch {
            // Share handoff must never crash the host app or extension.
        }
    }

    static func remove(fileName: String) {
        guard let url = AppGroup.fileURL(fileName) else { return }
        try? FileManager.default.removeItem(at: url)
    }
}

/// Snapshot of one of the user's joined communities, flat enough for the
/// extension to render a picker without pulling the Rust core in.
public struct SharedCommunitySummary: Codable, Hashable {
    public let id: String
    public let name: String
    public let picture: String

    public init(id: String, name: String, picture: String) {
        self.id = id
        self.name = name
        self.picture = picture
    }
}

/// The list of joined communities the main app last observed. The main app
/// writes on every refresh; the extension reads on launch.
public enum SharedCommunitiesSnapshot {
    private static let fileName = "joined-communities-v1.json"

    public static func load() -> [SharedCommunitySummary] {
        AppGroupJSONFiles.load([SharedCommunitySummary].self, fileName: fileName) ?? []
    }

    public static func save(_ snapshotData: Data) {
        AppGroupJSONFiles.saveData(snapshotData, fileName: fileName)
    }

    public static func clear() {
        AppGroupJSONFiles.remove(fileName: fileName)
    }
}

/// A share the user submitted in the extension but hasn't been published
/// yet — the main app drains this on foreground.
public struct PendingShare: Codable, Hashable, Identifiable {
    public let id: UUID
    public let groupId: String
    public let url: String
    public let note: String
    public let createdAt: Date

    public init(
        id: UUID = UUID(),
        groupId: String,
        url: String,
        note: String,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.groupId = groupId
        self.url = url
        self.note = note
        self.createdAt = createdAt
    }
}

public enum ShareQueue {
    private static let fileName = "pending-shares-v1.json"

    public static func enqueue(_ share: PendingShare) {
        var current = load()
        current.append(share)
        save(current)
    }

    public static func load() -> [PendingShare] {
        AppGroupJSONFiles.load([PendingShare].self, fileName: fileName) ?? []
    }

    public static func drain() -> [PendingShare] {
        let items = load()
        AppGroupJSONFiles.remove(fileName: fileName)
        return items
    }

    public static func replace(_ items: [PendingShare]) {
        save(items)
    }

    private static func save(_ items: [PendingShare]) {
        AppGroupJSONFiles.save(items, fileName: fileName)
    }
}

/// URL used by the extension to hand off control to the main app once a
/// share is enqueued. The main app's `.onOpenURL` handler recognizes this
/// and kicks off queue processing.
public enum ShareURLScheme {
    public static let scheme = "highlighter"
    public static let processShareHost = "process-share"

    public static var processShareURL: URL? {
        URL(string: "\(scheme)://\(processShareHost)")
    }

    public static func isProcessShare(_ url: URL) -> Bool {
        url.scheme == scheme && url.host == processShareHost
    }
}
