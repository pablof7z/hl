# iOS UI Preservation And Rust Projection Inventory
## Version 1.0 | June 8, 2026

This inventory belongs to the NMP/RMP mobile rebuild plan in `docs/plans/2026-06-08-nmp-rmp-mobile-rebuild.md`. It defines the current iOS UI surfaces that must be preserved while moving business logic to Rust. SwiftUI views may be rewired to Rust projections and dispatch-only actions, but their visible layout, copy, navigation shape, and interaction affordances are the baseline.

## Preservation Rule

For every row below:

- SwiftUI view composition stays visually equivalent.
- Swift `@State` may hold only ephemeral visual state such as focus, pressed state, sheet presentation toggles, text-field editing buffers before dispatch, scroll position, zoom gesture state, or OS handles.
- Swift must not own durable app facts, protocol facts, caches, retries, route decisions, publish stages, validation policy, relay choices, membership state, search timing, or error semantics.
- The Rust projection listed for the surface is the source of truth for rendered product state.
- User actions dispatch typed Rust actions. They do not call broad `get_*`, `publish_*`, `subscribe_*`, or throwing product methods.

## App Chrome And Navigation

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions |
|---|---|---|---|---|
| App launch and root scene | `App.swift`, `RootSceneView.swift` | `HighlighterStore.bootstrap`, `AppSessionStore.restoreSession`, scene-phase reconnect/disconnect policy, share drain decision, What's New seen state | `AppChromeSnapshot`, `LifecycleSnapshot`, `WhatsNewSnapshot`, `ShareIntakeSnapshot` | `AppStarted`, `LifecycleChanged`, `OpenUrlReceived`, `WhatsNewDismissed`, `ShareQueueDrainRequested` |
| Main tabs | `MainTabView.swift` | Swift tab default/semantic route ownership | `RootRouteSnapshot` | `RootTabSelected`, `GlobalCaptureTapped`, `MeRouteSelected` |
| Global toolbar | `GlobalUserToolbar.swift` | Logout routing, profile navigation decisions | `GlobalUserSnapshot`, `GlobalToolbarSnapshot` | `ProfileTapped`, `SettingsTapped`, `BookmarksTapped`, `LogoutRequested`, `LogoutConfirmed` |
| Shake feedback | `ShakeDetector.swift`, `FeedbackThreadsView.swift` | Feedback presentation policy | `FeedbackEntrySnapshot` | `FeedbackShortcutTriggered` |

## Auth And Onboarding

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions / Capabilities |
|---|---|---|---|---|
| Login | `LoginView.swift`, `KnownSigner.swift` | Input classification, NIP-46 start/pair policy, signer app selection, login errors, secret persistence decisions | `LoginSnapshot` | `LoginInputChanged`, `LoginSubmitted`, `SignerSelected`, `NostrConnectRequested`, `CapabilityResult::SignerCallback`, `CapabilityRequest::OpenUrl` |
| Account creation | `OnboardingCreateAccountView.swift` | Generated account persistence and error state | `AccountCreationSnapshot` | `CreateAccountTapped`, `GeneratedSecretAcknowledged`, `CapabilityRequest::SecureStoreWrite` |
| Onboarding intro/interests | `OnboardingView.swift`, `OnboardingWelcomeView.swift`, `OnboardingInterestsView.swift` | Onboarding completion, recommended follow publishing stage | `OnboardingSnapshot`, `InterestSelectionSnapshot` | `OnboardingAdvanced`, `InterestToggled`, `OnboardingCompleted` |
| Keys settings | `KeysView.swift`, `KeychainService.swift` | Secret reads and copy-state policy beyond ephemeral copied visual flag | `KeysSnapshot` | `RevealSecretRequested`, `CopySecretTapped`, `CapabilityRequest::SecureStoreRead` |

## Communities And Discovery

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions |
|---|---|---|---|---|
| Room explorer | `RoomExplorerView.swift`, `RoomExplorerStore.swift`, `ExplorerHeroView.swift`, `FriendsOnRoomCard.swift`, `RoomCoverCard.swift`, `RoomSquareTile.swift`, `RoomBrowseAllView.swift`, `RoomExplorerConfig.swift` | Featured curator config fetch/cache, discovery subscriptions, shelf sorting/filtering, join request state | `RoomExplorerSnapshot` | `ExplorerOpened`, `ExplorerRefreshRequested`, `RoomPreviewRequested`, `JoinRoomRequested` |
| Room preview | `RoomPreviewSheet.swift` | Preview refresh, join request, share target setup | `RoomPreviewSnapshot` | `RoomPreviewOpened`, `RoomPreviewJoinTapped`, `RoomPreviewShareTapped` |
| Room home | `RoomHomeView.swift`, `RoomLanesView.swift`, `RoomStore.swift`, room library card views, `ArtifactDetailView.swift` | Artifact/highlight fetches, room subscriptions, lane construction, comment/highlight counts | `RoomHomeSnapshot`, `RoomLaneSnapshot`, `ArtifactDetailSnapshot` | `RoomOpened`, `RoomClosed`, `RoomTabSelected`, `ArtifactOpened`, `ArtifactShared`, `RoomRefreshRequested` |
| Discussions | `DiscussionListView.swift`, `DiscussionDetailView.swift`, `DiscussionStore.swift`, `DiscussionComposerView.swift` | Discussion fetch/subscribe/publish, draft validation, error state | `RoomDiscussionsSnapshot`, `DiscussionDetailSnapshot`, `DiscussionComposerSnapshot` | `DiscussionComposerOpened`, `DiscussionDraftChanged`, `DiscussionSubmitted`, `DiscussionOpened` |
| Chat | `ChatView.swift`, `ChatStore.swift` | Chat fetch/subscribe, pagination, optimistic insert, presence probe | `RoomChatSnapshot` | `ChatOpened`, `ChatDraftChanged`, `ChatSendTapped`, `ChatLoadMoreTapped`, `ChatReplyTargetSelected` |
| Create room | `CreateRoomSheet.swift`, `RoomInviteView.swift`, `RoomShareCard.swift` | Photo upload policy, create-room publish stage, invite code minting, follows lookup, npub decode, member add, copied/sent toast timing | `CreateRoomSnapshot`, `RoomInviteSnapshot`, `RoomShareSnapshot` | `CreateRoomFieldChanged`, `CreateRoomPhotoSelected`, `CreateRoomSubmitted`, `InviteQueryChanged`, `InviteCandidateSelected`, `InviteMembersSubmitted`, `InviteCodeRequested` |

## Capture And Artifact Intake

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions / Capabilities |
|---|---|---|---|---|
| Capture flow | `CaptureFlow.swift`, `CapturePageView.swift`, `CaptureStore.swift`, `CaptureMetadataSheet.swift`, `CommunityPicker.swift` | Flow state machine, draft validation, community target state, publish stage, ISBN/artifact preview cache | `CaptureSnapshot`, `CaptureDraftSnapshot`, `CommunityPickerSnapshot` | `CaptureStarted`, `CaptureModeSelected`, `CaptureDraftChanged`, `CaptureCommunitySelected`, `CaptureSubmitted` |
| Book scanner | `BookScannerView.swift`, `BookScannerModel.swift`, `CameraView.swift`, `PageSegmentation.swift`, `ImageProcessing.swift` | Page segmentation policy, capture acceptance, camera lifecycle policy | `BookScannerSnapshot` | `ScannerOpened`, `ScannerFrameCaptured`, `ScannerImageAccepted`, `ScannerClosed`, `CapabilityRequest::CameraStart`, `CapabilityResult::CameraImage` |
| OCR | `OCRService.swift`, `OCRStructureReconstructor.swift` | Text reconstruction, confidence policy, quote/context derivation | `OcrReviewSnapshot` | `OcrRequested`, `OcrObservationSelected`, `CapabilityRequest::OcrRecognize`, `CapabilityResult::OcrObservations` |
| Book picker | `BookPicker.swift`, `BookSelection.swift`, `ISBNValidator.swift`, `ManualISBNEntryView.swift` | ISBN normalization/validation, recent books, artifact search, lookup debounce | `BookPickerSnapshot` | `BookPickerOpened`, `BookQueryChanged`, `IsbnEntered`, `BookSelected` |
| Share to community | `ShareToCommunitySheet.swift`, `ArtifactPreviewBuilder.swift`, `ShareQueueProcessor.swift`, share extension files, shared store files | Share parsing, queue drain policy, preview build, publish/share stages, joined-community mirror semantics | `ShareSheetSnapshot`, `ShareQueueSnapshot` | `ShareTargetSelected`, `ShareNoteChanged`, `ShareSubmitted`, `SharePayloadReceived`, `CapabilityResult::SharePayload` |

## Readers, Highlights, Comments, And Media

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions / Capabilities |
|---|---|---|---|---|
| Article reader | `ArticleReaderView.swift`, `ArticleReaderStore.swift`, `ArticleBodyView.swift`, `ArticleRowActions.swift`, `MarkdownRenderer.swift`, `FootnotePreprocessor.swift` | Article/highlight/profile fetches, subscription lifecycle, highlight publish stage | `ArticleReaderSnapshot` | `ArticleOpened`, `ArticleClosed`, `ArticleHighlightSelected`, `ArticleHighlightSubmitted`, `ArticleActionTapped` |
| Book view | `BookView.swift`, `BookTarget.swift` | Target resolution if any beyond route data | `BookSnapshot` | `BookOpened`, `BookArtifactActionTapped` |
| Web reader | `WebReaderView.swift`, `WebReaderTarget.swift` | Reader target policy and metadata handling | `WebReaderSnapshot` | `WebReaderOpened`, `WebReaderActionTapped` |
| Highlights feed/detail | `HighlightsTabView.swift`, `HomeFeedStore.swift`, `HighlightsStore.swift`, `HighlightFeedCardView.swift`, `HighlightDetailView.swift`, `HighlightPageImage.swift` | Following feed/highlights subscriptions, share/reaction/bookmark/comment state, nevent relay hints | `HighlightsHomeSnapshot`, `HighlightDetailSnapshot` | `HighlightsOpened`, `HighlightOpened`, `HighlightShared`, `HighlightReactionTapped`, `HighlightBookmarkTapped` |
| Comments | `CommentsSheet.swift`, `CommentsStore.swift`, `CommentComposer.swift`, `CommentRow.swift`, `CommentTreeBuilder.swift`, `CommentsToolbar.swift`, `CommentsAttachment.swift`, `ArtifactRef.swift` | Comment fetch/publish/thread building/focused-node state where semantic | `CommentsSnapshot` | `CommentsOpened`, `CommentDraftChanged`, `CommentSubmitted`, `CommentReplyTapped`, `CommentsClosed` |
| Podcast player | `MiniPlayerView.swift`, `PodcastListeningView.swift`, `PodcastPlayerStore.swift`, `ClipComposerSheet.swift`, `ClipTimelineView.swift`, podcast row views, `TranscriptParser.swift`, `TranscriptView.swift`, `WaveformExtractor.swift` | Playback state, resume policy, transcript parsing policy, waveform cache policy, clip range state, comments cache, publish stage | `PodcastPlayerSnapshot`, `PodcastListeningSnapshot`, `ClipComposerSnapshot` | `PodcastLoaded`, `AudioPlayTapped`, `AudioPauseTapped`, `AudioSeekRequested`, `AudioProgressReported`, `ClipComposerOpened`, `ClipRangeChanged`, `ClipSubmitted`, `CapabilityRequest::AudioStart`, `CapabilityResult::AudioProgress` |

## Personal Library, Search, Profile, Settings, Feedback

| Surface | Current Swift Files | Current Business Owners To Remove | Rust Projection | Rust Actions / Capabilities |
|---|---|---|---|---|
| Reads | `ReadsStore.swift`, `ReadingCard.swift`, `ReadingFeedCardView.swift` | Following reads fetch/subscribe | `ReadsSnapshot` | `ReadsOpened`, `ReadItemOpened` |
| Bookmarks/vault | `BookmarksView.swift`, `BookmarkStore.swift`, `BookmarkMenuButton.swift`, `SetDetailView.swift` | Bookmark sets, web bookmarks, optimistic membership, create set state | `BookmarksSnapshot`, `BookmarkMenuSnapshot`, `SetDetailSnapshot` | `BookmarksOpened`, `BookmarkToggled`, `BookmarkSetCreated`, `BookmarkSetMembershipChanged`, `BookmarkFilterSelected` |
| Search | `SearchView.swift`, `SearchSeeAllView.swift`, `SearchStore.swift`, `RecentSearches.swift` | Search debounce, recent searches, relay loading timeout, search relay list, local/relay merge | `SearchSnapshot`, `SearchSeeAllSnapshot` | `SearchQueryChanged`, `SearchSubmitted`, `SearchCleared`, `RecentSearchSelected`, `SearchSeeAllOpened` |
| Profile | `ProfileView.swift`, `ProfileStore.swift`, `EditProfileSheet.swift`, `ArticleCardView.swift`, `CommunityRowView.swift`, `AuthorAvatar.swift` | Profile/articles/highlights/communities fetch, follow state, edit publish stage | `ProfileSnapshot`, `EditProfileSnapshot`, `AuthorSnapshot` | `ProfileOpened`, `FollowToggled`, `EditProfileOpened`, `ProfileFieldChanged`, `ProfileSubmitted` |
| Settings | `SettingsView.swift`, `MediaSettingsView.swift`, network settings files | Relay role persistence, Wi-Fi-only setting, NIP-11 probe coalescing, Blossom server list, import relays, error state | `SettingsSnapshot`, `MediaSettingsSnapshot`, `NetworkSettingsSnapshot`, `RelayDetailSnapshot` | `SettingsOpened`, `BlossomServerAdded`, `BlossomServerRemoved`, `RelayAdded`, `RelayRemoved`, `RelayRolesChanged`, `RelayImportRequested`, `WifiOnlyChanged` |
| Feedback | feedback views/stores | Thread fetch/subscribe/publish, project coordinate policy, first-agent lookup | `FeedbackThreadsSnapshot`, `FeedbackThreadSnapshot`, `FeedbackComposerSnapshot` | `FeedbackOpened`, `FeedbackThreadOpened`, `FeedbackDraftChanged`, `FeedbackSubmitted` |
| Rich Nostr text | `NostrRichText.swift` | Entity resolve/subscribe/profile fetches | `RichTextEntitySnapshot` scoped by renderer host view | `NostrEntityTapped`, `NostrEntityVisible`, `NostrEntityClosed` |
| Image zoom and pure visual components | `ImageZoomView.swift`, design color files, pure row/card views | None if they stay visual-only | Parent projection | Pure Swift gesture state allowed |

## Swift Files Expected To Disappear Or Become Capability-Only

These files currently own business behavior and should not survive as product state owners:

- `Core/HighlighterStore.swift`
- `Core/SafeHighlighterCore.swift`
- `Core/EventBridge.swift`
- `Session/AppSessionStore.swift`
- `Core/RoomExplorerConfig.swift`
- `Core/BlossomUploadService.swift` unless converted into a raw upload capability executor.
- `Core/WhatsNew.swift` unless converted into pure resource decoding with seen state in Rust.
- `Features/*/*Store.swift`
- `Features/Search/RecentSearches.swift`
- `Features/Share/ShareQueueProcessor.swift` unless converted into raw share payload delivery.
- `Features/Capture/OCRStructureReconstructor.swift`, `OCRService.swift`, `PageSegmentation.swift`, `ImageProcessing.swift` unless converted into OS/raw image helpers with reconstruction in Rust.
- `Features/Podcast/TranscriptParser.swift`, `WaveformExtractor.swift`, and playback policy portions of `PodcastPlayerStore.swift`.

## Allowed Native State

Swift and Kotlin may retain:

- Text-field editing buffers before dispatch, provided validation and submission state come from Rust.
- Focus, scroll position, selection highlight, pressed/hovered state, local sheet presentation mechanics, and animation state.
- Gesture state such as image zoom scale and drag offset.
- Transient OS handles for camera sessions, audio players, file pickers, URL openers, signer intents, and secure storage calls.
- Generated UniFFI binding internals.

Any native state that needs to survive view dismissal, affect sorting/filtering/routing/publish semantics, or be shared by another screen belongs in Rust.

## Baseline Verification Status

Initial baseline commands started on June 8, 2026:

- `cd app/core && cargo test` started; compiling dependencies.
- `xcodebuild -list -project app/ios/Highlighter/Highlighter.xcodeproj` passed and listed `Highlighter` and `HighlighterShareExtension` schemes.
- Available simulator target found: `iPhone 17 Pro` on iOS 26.2, already booted.

The next gate is an iOS simulator build and screenshot baseline capture after Rust dependencies finish compiling.
