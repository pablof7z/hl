import SwiftUI

struct ClipComposerSheet: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    let artifact: ArtifactRecord

    @Binding var startSeconds: Double
    @Binding var endSeconds: Double

    @State private var note: String = ""
    @State private var selectedGroupId: String?
    @State private var showCommunityPicker = false
    @State private var isPublishing = false
    @State private var publishError: String?

    private var player: PodcastPlayerStore { app.podcastPlayer }

    // MARK: - Computed

    private var composerProjection: PodcastClipComposerProjection {
        app.core.getPodcastClipComposerProjection(input: PodcastClipComposerInput(
            segments: player.transcriptSegments,
            transcriptAvailable: player.transcriptAvailability == .available,
            clipStartSeconds: startSeconds,
            clipEndSeconds: endSeconds,
            durationSeconds: player.duration,
            selectedGroupId: selectedGroupId,
            joinedCommunities: app.joinedCommunities
        ))
    }

    private var extractedFragment: String {
        composerProjection.excerpt
    }

    private var canPublish: Bool {
        composerProjection.canPublish && !isPublishing
    }

    private var communityName: String {
        composerProjection.communityDisplayName
    }

    private var hasCommunity: Bool {
        composerProjection.hasCommunity
    }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                header
                    .padding(.horizontal, 20)
                    .padding(.top, 4)
                    .padding(.bottom, 16)

                Divider()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        excerptSlot
                        noteField
                        roomPickerRow
                        if let err = publishError {
                            Text(err)
                                .font(.footnote)
                                .foregroundStyle(.red)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        actionsRow
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 16)
                }
            }
            .presentationDetents([.medium, .large])
            .presentationDragIndicator(.visible)
            .sheet(isPresented: $showCommunityPicker) {
                CommunityPicker(selection: $selectedGroupId)
                    .environment(app)
            }
        }
        .onAppear {
            if selectedGroupId == nil && !artifact.groupId.isEmpty {
                selectedGroupId = artifact.groupId
            }
        }
    }

    // MARK: - Header

    private var header: some View {
        VStack(spacing: 12) {
            Text("New Clip")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .center)

            rangeRow
        }
    }

    private var rangeRow: some View {
        let projection = composerProjection

        return VStack(spacing: 6) {
            HStack(spacing: 0) {
                timeEditor(label: projection.clipStartLabel, direction: .leading) { delta in
                    let proposed = startSeconds + delta
                    startSeconds = max(0, min(endSeconds - 5, proposed))
                }

                Spacer(minLength: 0)

                Text("→")
                    .font(.title3.weight(.light))
                    .foregroundStyle(.secondary)

                Spacer(minLength: 0)

                timeEditor(label: projection.clipEndLabel, direction: .trailing) { delta in
                    let proposed = endSeconds + delta
                    endSeconds = max(startSeconds + 5, min(player.duration > 0 ? player.duration : proposed, proposed))
                }
            }

            Text(projection.subtitleLabel)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .center)
        }
    }

    private enum NudgeAlignment { case leading, trailing }

    private func timeEditor(
        label: String,
        direction: NudgeAlignment,
        onNudge: @escaping (Double) -> Void
    ) -> some View {
        HStack(spacing: 8) {
            if direction == .trailing {
                nudgeButton(label: "-5s", delta: -5, onNudge: onNudge)
            }

            Text(label)
                .font(.system(size: 24, weight: .semibold).monospacedDigit())
                .foregroundStyle(.primary)

            if direction == .leading {
                nudgeButton(label: "+5s", delta: +5, onNudge: onNudge)
            }
        }
    }

    private func nudgeButton(label: String, delta: Double, onNudge: @escaping (Double) -> Void) -> some View {
        Button {
            onNudge(delta)
        } label: {
            Text(label)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Color(.tertiarySystemFill), in: Capsule())
        }
        .buttonStyle(.plain)
    }

    // MARK: - Excerpt slot

    @ViewBuilder
    private var excerptSlot: some View {
        if !extractedFragment.isEmpty {
            HStack(alignment: .top, spacing: 0) {
                Rectangle()
                    .fill(Color.highlighterAccent)
                    .frame(width: 3)
                    .clipShape(RoundedRectangle(cornerRadius: 2))

                Text(extractedFragment)
                    .font(.system(.callout, design: .default).italic())
                    .foregroundStyle(.primary)
                    .lineSpacing(6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 10)
            }
            .background(Color(.secondarySystemFill), in: RoundedRectangle(cornerRadius: 8))
        } else {
            VStack(alignment: .leading, spacing: 6) {
                Text("No transcript in range")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)

                Text(composerProjection.timeOnlyMessage)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 8))
        }
    }

    // MARK: - Note field

    private var noteField: some View {
        TextField("Add a note for the room…", text: $note, axis: .vertical)
            .lineLimit(1...4)
            .font(.callout)
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 12))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .strokeBorder(Color(.separator), lineWidth: 1)
            )
    }

    // MARK: - Room picker

    private var roomPickerRow: some View {
        Button {
            showCommunityPicker = true
        } label: {
            HStack(spacing: 10) {
                Image(systemName: "number")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .frame(width: 20)
                Text("Room")
                    .font(.callout)
                    .foregroundStyle(.primary)
                Spacer()
                Text(communityName)
                    .font(.callout)
                    .foregroundStyle(hasCommunity ? Color.highlighterAccent : Color.secondary)
                    .lineLimit(1)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(Color(.secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color(.separator), lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    // MARK: - Actions

    private var actionsRow: some View {
        HStack(spacing: 12) {
            Button("Cancel") {
                dismiss()
            }
            .font(.body.weight(.medium))
            .foregroundStyle(.primary)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(Color(.secondarySystemFill), in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Color(.separator), lineWidth: 1))
            .buttonStyle(.plain)

            Button {
                publishClip()
            } label: {
                Group {
                    if isPublishing {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Publish")
                            .font(.body.weight(.semibold))
                            .foregroundStyle(.white)
                    }
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(
                    canPublish ? Color.highlighterAccent : Color.highlighterAccent.opacity(0.4),
                    in: RoundedRectangle(cornerRadius: 14)
                )
            }
            .buttonStyle(.plain)
            .disabled(!canPublish)
        }
    }

    // MARK: - Publish

    private func publishClip() {
        guard canPublish else { return }
        isPublishing = true
        publishError = nil

        Task {
            let outcome = await app.core.publishPodcastComposerClip(
                input: PodcastClipComposerPublishInput(
                    artifact: artifact,
                    segments: player.transcriptSegments,
                    transcriptAvailable: player.transcriptAvailability == .available,
                    context: note,
                    clipStartSeconds: startSeconds,
                    clipEndSeconds: endSeconds,
                    targetGroupId: selectedGroupId
                )
            )
            await MainActor.run {
                isPublishing = false
                let result = app.core.projectPodcastClipPublishResult(
                    input: PodcastClipPublishResultInput(snapshot: outcome)
                )
                if !result.didPublish {
                    publishError = result.errorMessage
                } else {
                    if let toast = result.shareToast {
                        app.shareToast = toast
                    }
                    if result.shouldDismiss {
                        dismiss()
                    }
                }
            }
        }
    }
}
