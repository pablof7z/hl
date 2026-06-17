import Foundation

/// What the user has chosen to highlight from. Either an artifact that's
/// already been shared (kind:11 exists on relay), or a preview we'll publish
/// on their behalf the moment they hit Publish.
///
/// Carrying both cases through the store keeps the publish path unified: the
/// picker never has to decide "have I shared this yet?" — it just hands the
/// store a `BookSelection` and the store resolves the kind:11 side at
/// publish time.
enum BookSelection: Equatable {
    case existing(ArtifactRecord)
    case pending(ArtifactPreview)

    var preview: ArtifactPreview {
        switch self {
        case .existing(let record): return record.preview
        case .pending(let preview): return preview
        }
    }
}

