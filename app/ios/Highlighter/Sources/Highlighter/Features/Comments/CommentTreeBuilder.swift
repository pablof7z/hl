typealias CommentNode = CommentThreadNode

extension CommentThreadNode: Identifiable {
    public var id: String { record.eventId }
}
