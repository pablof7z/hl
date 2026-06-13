# Android ↔ iOS Feature-Parity Plan (Highlighter)

**Status:** Planning / review deliverable. No app code is changed by this document.
**Date:** 2026-06-13
**Author:** automated audit (Opus 4.8)
**Scope:** Bring the Android (Kotlin/Compose) client to feature parity with the iOS (SwiftUI) reference client, and define a validation harness so each phase is proven by actually running the app against real Nostr data.

---

## 0. Architecture grounding (read this first)

Both apps are **thin renderers over a shared Rust core** exposed via UniFFI. The core is a single state machine:

- One object: `HighlighterNmpApp` (iOS: `HighlighterNmpApp`; Kotlin binding: `uniffi.highlighter_core.HighlighterNmpApp`).
- One **action channel**: `app.dispatch(action: HighlighterAppAction)` — an enum of ~140 cases (the full list is in §App Contract below).
- One **state tree**: `app.state() -> HighlighterAppState`, a snapshot containing one child snapshot per screen (`homeFeed`, `roomExplorer`, `roomDetail`, `createRoom`, `roomInvite`, `comments`, `capture`, `bookPicker`, `search`, `profileView`, `articleReader`, `bookmarks`, `network`, `auth`, `onboarding`, `feedback`, `shareComposer`, `editProfile`, `mediaSettings`, `curationMenu`, `whatsNew`, `chrome`, `toast`, plus list buffers `profiles`, `isbnPreviews`, `webMetadata`, `referenceHighlights`).
- Push updates: `app.listenForUpdates(reconciler)` delivers new `HighlighterAppState` snapshots; `app.setCoreEventCallback(callback)` delivers deltas (NIP-46 connect, etc.).
- A handful of async escape-hatch methods: `article(pubkeyHex:dTag:)`, `decodeNostrEntity(input:)`, `resolveNostrEntity(entity:)`, `highlightShareUrl(...)`, `publishUrlShare(...)`, `networkRemovalImpact(url:)`.
- Free functions: `normalizeIsbn(raw:)`, `initPlatformLogging()`.

**Consequence for parity work:** Android does **not** need new business logic. Every feature iOS has already exists in the core as actions + snapshot fields. Parity = render the snapshot child + dispatch the right actions + correct navigation/IA. The generated Kotlin binding is at:
`app/android/app/build/generated/source/uniffi/main/kotlin/uniffi/highlighter_core/highlighter_core.kt`
(grep it to confirm any action/type name before using it).

**Key correction to the premise:** The Android app is **not** a non-functional shell. The bridge (`HighlighterViewModel`), the auth/onboarding gate (`RootScene`), the three-tab scaffold (`MainScaffold`), the home feed, room explorer, room detail, profile, bookmarks, search, reader, settings, and podcast are all wired and render real snapshots. The user's "nothing works" experience is produced by a **small number of high-impact defects** (one IA bug, one teardown bug, one large feature gap, and data/relay timing) layered on an otherwise-correct app — see §3 Gap Audit. The plan therefore prioritizes surgical fixes over a rewrite.

---

## App Contract — full `HighlighterAppAction` surface

These are the dispatchable actions (verbatim from the generated binding). Android parity work must use these exact actions; do not invent new ones.

```
bootstrap, refreshAppChrome, appForegrounded
signInNsec(nsec,persist,clearStoredOnFailure), pairBunker(uri,...), startNostrConnect(callbackUrl), logout, externalUrlOpenFailed(url)
setCreateAccountDisplayName, setCreateAccountUsername, submitCreateAccount
toggleOnboardingInterest(interestId), completeOnboarding
uploadCreateRoomCover, clearCreateRoomCover, createRoomCapabilityFailed, submitCreateRoom(name,about,visibility,access), clearCreateRoomResult, clearCreateRoomError
openRoomInvite(groupId), refreshRoomInvite, setRoomInviteQuery, toggleRoomInviteCandidate, removeRoomInviteCandidate, acceptRoomInvitePastedCandidate, mintRoomInviteLink, submitRoomInviteMembers, clearRoomInviteAddError, clearRoomInviteInviteLinkError, clearRoomInviteToast, closeRoomInvite
openComments(rootTagName,rootTagValue,rootKind), refreshComments, setCommentDraft(parentEventId?,body), publishComment(parentEventId?), clearCommentPublishError, toggleCommentLike, toggleCommentBookmark, clearCommentInteractionError, closeComments
openFeedback(coordinate), refreshFeedbackThreads, setFeedbackNewThreadDraft, publishFeedbackNewThread, openFeedbackThread, refreshFeedbackThread, setFeedbackReplyDraft, publishFeedbackReply, clearFeedbackPublishError, closeFeedbackThread, closeFeedback
openMediaSettings, refreshMediaSettings, addBlossomServer, removeBlossomServer, moveBlossomServers, clearMediaSettingsError, closeMediaSettings
openEditProfile(seed?), setEditProfile{DisplayName,Name,About,Picture,Banner,Nip05,Website,Lud16}, uploadEditProfileImage, editProfileCapabilityFailed, submitEditProfile, clearEditProfileError, clearEditProfileResult, closeEditProfile
toggleArticleBookmark(address)
openBookmarks, refreshBookmarks, closeBookmarks, openBookmarkCollection(pubkeyHex,dTag,kind), refreshBookmarkCollection
openCurationMenu(articleAddress), closeCurationMenu, setAddressInCurationSet, createCurationSetAndAdd
openRoomExplorer, refreshRoomExplorer, refreshRoomBrowseAll, requestJoinRoom(groupId,roomName)
requestIsbnPreview(isbn), requestWebMetadata(url), requestReferenceHighlights(tagName,tagValue,limit)
requestBookPickerRecents(limit), searchBookPickerArtifacts(query,limit), clearBookPickerSearch
uploadCapturePhoto(bytes,mime,w,h,alt), clearCaptureUpload, publishCaptureHighlight(selection,targetGroupId?,draft), publishCapturePicture(selection?,targetGroupId?,image,note), publishClipHighlight(artifact,targetGroupId?,draft), clearCaptureResult, clearCaptureError
requestProfile(pubkeyHex), openProfile(pubkeyHex), refreshProfile, closeProfile, toggleProfileFollow
openArticleReader(pubkeyHex,dTag,seed?), refreshArticleReader, closeArticleReader, publishArticleHighlight(quote,context,note)
publishArtifactShare(preview,groupId,note?), publishUrlShare(url,groupId,note?), shareHighlightRepost(eventId,authorPubkeyHex,relayHint,targetGroupId), clearShareComposerResult, clearShareComposerError
openRoom(groupId), refreshRoom, publishRoomDiscussion(title,body,attachmentUrl?), clearRoomDiscussionError, loadMoreRoomChat, publishRoomChatMessage(content,replyToEventId?), clearRoomChatError, closeRoom
openHomeFeed, refreshHomeFeed, closeHomeFeed
searchOpened, searchClosed, setSearchQuery, submitSearch, clearSearch, recordRecentSearch, clearRecentSearches
openNetworkSettings, refreshNetworkSettings, upsertNetworkRelay, removeNetworkRelay, setNetworkRelayRoles, probeNetworkRelayNip11, setNetworkImportNpub, fetchNetworkImportRelays, toggleNetworkImportRelay, applyNetworkImportRelays, clearNetworkError, closeNetworkSettings, setNetworkWifiOnly, networkPathChanged, reconnectNetwork
dismissWhatsNew, clearToast
```

> Note: actions exist in the binding even if neither platform calls them yet. The presence of `closeRoom` vs an explorer-close is relevant to a bug in §3.

---

## 1. iOS Feature Inventory (reference)

Base: `app/ios/Highlighter/Sources/Highlighter`. Store is injected as `@Environment(HighlighterStore.self)` (bound as `app`/`store`/`appStore`); methods like `store.openHomeFeed()` wrap `dispatch(.openHomeFeed)`.

### Shell / Navigation
| Feature | iOS file(s) | Core actions / snapshot |
|---|---|---|
| Auth/onboarding gate | `Navigation/RootSceneView.swift` | reads `isLoggedIn`, `nmpState.onboarding.isComplete`; `bootstrap()`, `appForegrounded()` |
| 3-tab shell (Highlights / Rooms / Search) | `Navigation/MainTabView.swift` | tab roots below |
| Global user menu (Profile / Bookmarks / Settings / Log out) | `Navigation/GlobalUserToolbar.swift` | `logout`, opens Profile/Bookmarks/Settings |
| Profile nav value | `Navigation/ProfileDestination.swift` | `openProfile(pubkeyHex)` |
| Deep-link / share-link routing | `Navigation/ShareLinkRouter.swift` | `decodeNostrEntity`, `openProfile/openComments/openArticleReader` |
| Mini player accessory | `MainTabView` + `Features/Podcast/MiniPlayerView.swift` | `podcastPlayer.currentArtifact` |
| Shake → feedback | `RootSceneView` + `Core/ShakeDetector.swift` | opens `FeedbackThreadsView` |

### Feed / Highlights (Home)
| Feature | iOS file(s) | Core |
|---|---|---|
| Home feed (mixed highlights + reads, chronological) | `Features/Highlights/HighlightsTabView.swift` | `openHomeFeed/refreshHomeFeed/closeHomeFeed`; reads `homeFeed.items` (`HighlighterHomeFeedItem` kind=`highlights`/`read`) |
| Grouped-highlight module (shared w/ rooms) | `Features/Highlights/HighlightFeedCardView.swift` | `requestIsbnPreview`, `article(...)`, `requestWebMetadata`, `requestProfile`; reads `webMetadata`, `profiles`, `isbnPreviews` |
| Highlight detail (quote-centric, share/comment/bookmark) | `Features/Highlights/HighlightDetailView.swift` | `openComments`, `highlightShareUrl`, `shareHighlightRepost` |
| Scanned-page image render | `Features/Highlights/HighlightPageImage.swift` | NIP-92 imeta image |
| Reading card (article surfaced by friends) | `Features/Reads/ReadingFeedCardView.swift`, `ReadingCard.swift` | reads `homeFeed` read items; `requestProfile` |

### Rooms / Communities (NIP-29)
| Feature | iOS file(s) | Core |
|---|---|---|
| Rooms explorer (hero + shelves + browse-all) | `Features/Communities/RoomExplorerView.swift` | `openRoomExplorer/refreshRoomExplorer`; reads `roomExplorer.{featured,friendsShelf,authorsShelf,newNoteworthy,allRooms}` |
| Room preview / peek-inside sheet | `Features/Communities/RoomPreviewSheet.swift` | `openRoom`(peek), `requestJoinRoom`, `closeRoom` |
| Browse all (grid + search) | `Features/Communities/RoomBrowseAllView.swift` | `refreshRoomBrowseAll`; `roomExplorer.allRooms` |
| Room home (pill tabs: Home/Library/Discussions/Chat) | `Features/Communities/RoomHomeView.swift` | `openRoom/closeRoom`; reads `roomDetail.*` |
| Room lanes (artifact-grouped highlights) | `Features/Communities/RoomLanesView.swift` | `roomDetail.{highlightsByReference,commentsByReference}` |
| Group chat (NIP-29 kind:9) | `Features/Communities/ChatView.swift` | `publishRoomChatMessage`, `loadMoreRoomChat`, `clearRoomChatError` |
| Discussions list / detail / composer (kind:11) | `Features/Communities/DiscussionListView.swift`, `DiscussionDetailView.swift`, `DiscussionComposerView.swift` | `publishRoomDiscussion`, `openComments(kind:11)` |
| Artifact detail router | `Features/Communities/ArtifactDetailView.swift` | routes podcast/article/book |
| Create room (modal sheet) | `Features/Communities/CreateRoom/CreateRoomSheet.swift` | `uploadCreateRoomCover`, `submitCreateRoom`, `clearCreateRoomResult/Error` |
| Invite people (welcome/manage) | `Features/Communities/CreateRoom/RoomInviteView.swift`, `RoomShareCard.swift` | `openRoomInvite`, `setRoomInviteQuery`, `toggleRoomInviteCandidate`, `mintRoomInviteLink`, `submitRoomInviteMembers` |
| Room cards/tiles | `RoomCoverCard.swift`, `RoomSquareTile.swift`, `ExplorerHeroView.swift`, `FriendsOnRoomCard.swift`, `RoomLibrary{Article,Book,Podcast}CardView.swift` | presentational |

### Reading / Books
| Feature | iOS file(s) | Core |
|---|---|---|
| NIP-23 article reader + inline highlight | `Features/Article/ArticleReaderView.swift`, `ArticleBodyView.swift`, `MarkdownRenderer.swift`, `FootnotePreprocessor.swift`, `ArticleRowActions.swift` | `openArticleReader`, `publishArticleHighlight`, `toggleArticleBookmark` |
| Book detail + passages | `Features/Book/BookView.swift`, `BookTarget.swift` | `requestIsbnPreview`, `requestReferenceHighlights(tagName:"i",tagValue:"isbn:…")` |
| Web reader (Readability) | `Features/Web/WebReaderView.swift`, `WebReaderTarget.swift` | `requestWebMetadata` |

### OCR Capture (physical book → highlight)
| Feature | iOS file(s) | Core |
|---|---|---|
| Capture flow orchestrator | `Features/Capture/CaptureFlow.swift`, `CaptureStore.swift` | `uploadCapturePhoto`, `publishCaptureHighlight`, `publishCapturePicture`, `clearCapture*` |
| Document camera | `Features/Capture/CameraView.swift` (VNDocumentCameraViewController) | — |
| OCR engine + structure | `Features/Capture/OCRService.swift`, `OCRStructureReconstructor.swift`, `PageSegmentation.swift`, `ImageProcessing.swift` | on-device Vision → markdown + alt text |
| Page review + drag-select | `Features/Capture/CapturePageView.swift` | builds `HighlightDraft` |
| Metadata / destination sheet | `Features/Capture/CaptureMetadataSheet.swift` | book + room + note |
| Book picker + ISBN | `Features/Capture/BookPicker.swift`, `BookScannerView.swift`, `BookScannerModel.swift`, `ManualISBNEntryView.swift`, `ISBNValidator.swift`, `BookSelection.swift` | `requestBookPickerRecents`, `searchBookPickerArtifacts`, `requestIsbnPreview`, `normalizeIsbn` |
| Community picker | `Features/Capture/CommunityPicker.swift` | reads `chrome.joinedCommunities` |

### Profiles
| Feature | iOS file(s) | Core |
|---|---|---|
| Profile (hero, tabs: Writing/Highlights/Communities, follow) | `Features/Profile/ProfileView.swift` | `openProfile/closeProfile/refreshProfile/toggleProfileFollow` |
| Edit profile | `Features/Profile/EditProfileSheet.swift` | `openEditProfile`, `setEditProfile*`, `uploadEditProfileImage`, `submitEditProfile` |
| Components | `ArticleCardView.swift`, `CommunityRowView.swift`, `AuthorAvatar.swift`, `Components/NostrUser/*` | presentational |

### Composing / Sharing
| Feature | iOS file(s) | Core |
|---|---|---|
| Share to community (artifact/url/highlight-repost) | `Features/Share/ShareToCommunitySheet.swift`, `ArtifactPreviewBuilder.swift` | `publishArtifactShare`, `publishUrlShare`, `shareHighlightRepost` |
| Share-extension queue drain | `Features/Share/ShareQueueProcessor.swift`, `Sources/ShareExtension/*`, `Sources/Shared/SharedStore.swift` | `publishUrlShare` |
| Comments (NIP-22 threads) | `Features/Comments/*` (`CommentsSheet`, `ThreadView`, `CommentComposer`, `CommentRow`, `CommentTreeBuilder`, `ArtifactRef`, `CommentsAttachment`) | `openComments`, `setCommentDraft(parentEventId?)`, `publishComment(parentEventId?)`, `toggleCommentLike/Bookmark` |

### Auth / Onboarding
| Feature | iOS file(s) | Core |
|---|---|---|
| Login (signer detect, Primal/NostrConnect, nsec, bunker) | `Features/Auth/LoginView.swift`, `KnownSigner.swift` | `signInNsec`, `pairBunker`, `startNostrConnect` |
| Onboarding (welcome → create account → interests) | `Features/Auth/OnboardingView.swift`, `OnboardingWelcomeView.swift`, `OnboardingCreateAccountView.swift`, `OnboardingInterestsView.swift` | `setCreateAccount*`, `submitCreateAccount`, `toggleOnboardingInterest`, `completeOnboarding` |
| Keychain | `Session/KeychainService.swift`, `AppSessionStore.swift` | nsec persistence |

### Podcasts
| Feature | iOS file(s) | Core |
|---|---|---|
| Player + timeline (transcript/chapters/clips/waveform) | `Features/Podcast/PodcastListeningView.swift`, `PodcastPlayerStore.swift`, `Rows/*`, `TranscriptView.swift`, `TranscriptParser.swift`, `WaveformExtractor.swift` | playback local; clips via core |
| Clip composer → highlight | `Features/Podcast/ClipComposerSheet.swift` | `publishClipHighlight` |
| Mini player | `Features/Podcast/MiniPlayerView.swift` | — |

### Notifications / Feedback
| Feature | iOS file(s) | Core |
|---|---|---|
| In-app feedback (shake → threads, kind:1 under 31933) | `Features/Feedback/FeedbackThreadsView.swift`, `FeedbackThreadDetailView.swift`, `FeedbackNewThreadView.swift`, `FeedbackProject.swift` | `openFeedback`, `publishFeedbackNewThread`, `publishFeedbackReply` |
| What's New | `Features/WhatsNew/WhatsNewSheet.swift` | `dismissWhatsNew`; reads `whatsNew` |
| Toasts | `RootSceneView` ShareToastBanner | reads `toast`; `clearToast` |

> There is **no push-notification feature** in iOS either; "Notifications" parity = feedback + toasts + What's New only.

### Settings / Search / Discovery
| Feature | iOS file(s) | Core |
|---|---|---|
| Settings root | `Features/Settings/SettingsView.swift` | `logout` |
| Keys (nsec/npub) | `Features/Settings/KeysView.swift` | `KeychainService` |
| Media (Blossom servers) | `Features/Settings/MediaSettingsView.swift` | `openMediaSettings`, `addBlossomServer`, `removeBlossomServer`, `moveBlossomServers` |
| Network (relays, NIP-11, import, wifi-only, reconnect) | `Features/Settings/Network/*` (`NetworkSettingsView`, `AddRelaySheet`, `ImportRelaysSheet`, `RelayDetailView`, `RelayRowView`) | `openNetworkSettings`, `upsertNetworkRelay`, `removeNetworkRelay`, `setNetworkRelayRoles`, `probeNetworkRelayNip11`, `fetch/applyNetworkImportRelays`, `setNetworkWifiOnly`, `reconnectNetwork` |
| Search (sectioned: highlights/articles/communities/people + see-all) | `Features/Search/SearchView.swift`, `SearchSeeAllView.swift`, `SearchStore.swift` | `searchOpened/closed`, `setSearchQuery`, `submitSearch`, `clearRecentSearches` |
| Bookmarks / curation sets | `Features/Bookmarks/BookmarksView.swift`, `SetDetailView.swift`, `BookmarkMenuButton.swift` | `openBookmarks`, `openBookmarkCollection`, `openCurationMenu`, `setAddressInCurationSet`, `createCurationSetAndAdd` |

### Zaps / payments
Not present in iOS. Lightning address (`lud16`) is captured in Edit Profile only; there is **no zap send/receive UI** on either platform. **Out of scope for parity.**

---

## 2. User-Flow Catalog (validation test cases)

Each flow is a validation case for the harness (§5). "Expected (real data)" is what a Haiku validator must observe on screen. Flows assume the test account (§5.4) is logged in unless stated.

> Convention: where iOS and Android diverge today, the **Expected** describes the *target* (iOS-parity) behavior the Android validation must reach.

### Auth & onboarding
1. **Login with nsec** — Goal: authenticate. Steps: launch app logged-out → Login screen → paste test `nsec1…` → tap *Continue/Login*. Expected: gate advances to the 3-tab shell; top bar shows the test account avatar; status line settles to "Connected".
2. **Cold launch resumes session** — Goal: persistence. Steps: kill app → relaunch. Expected: no login screen; lands directly on Highlights with avatar present (credential restored from `SessionStore`).
3. **Onboarding interests (new login path)** — Goal: complete onboarding. Steps: from a fresh login where onboarding incomplete → interest chips screen → select ≥3 chips → *Continue*. Expected: chips toggle visibly; *Continue* enabled; tapping advances to Highlights.
4. **Create account (signup)** — Goal: new identity. Steps: Welcome → Create account → enter display name + username → submit. Expected: username availability indicator updates live; on submit, advances to interests then shell. *(Known risk: account-creation can hang on flaky network — see Known Issues #1; validate on a healthy network or treat a >30s spinner as a known core defect, not an Android bug.)*
5. **Log out** — Goal: sign out. Steps: top-bar avatar/menu → Settings (or menu) → Log Out → confirm. Expected: returns to Login screen; relaunch stays logged out.

### Feed / Highlights
6. **See the highlights feed** *(user's #1 complaint)* — Goal: view home feed. Steps: open app on Highlights tab; wait for sync. Expected: feed populates with ≥1 card; a highlight card shows a resource header (cover/title/author) + a serif pull-quote (or scanned page image); a reading card shows article title + author avatar + social badge ("…liked this" / "From someone you follow"). Empty state "No highlights yet" only if account truly has no feed.
7. **Pull-to-refresh feed** — Steps: on Highlights, pull down. Expected: refresh spinner; feed reloads (same or newer items), no crash.
8. **Open a highlight detail** — Steps: tap a highlight card. Expected: detail screen with the quote center-stage, byline (tappable → profile), action bar (comment / share / bookmark for articles).
9. **Open the source behind a highlight** — Steps: in highlight detail, tap the resource header. Expected: navigates into the article reader / book / web reader for that source.
10. **Open a reading card's article** — Steps: tap a reading (article) card in the feed. Expected: NIP-23 article reader opens with rendered body and author header.

### Rooms
11. **See the rooms explorer** — Goal: browse rooms. Steps: tap Rooms tab. Expected: explorer surface with a featured hero and shelves (Your rooms / Friends are here / Featured / New & noteworthy / Browse all). **No create-room form at the top** (target IA). At least one room tile renders a cover + name.
12. **Open a room** *(user's complaint "opening rooms does nothing")* — Steps: tap a room tile (joined room or preview→open). Expected: a room screen opens (overlay/detail) showing the room name and content; for joined rooms, the room home with pill tabs Home/Library/Discussions/(Chat). Tapping must *navigate*, not no-op.
13. **Room preview & join** — Steps: tap a non-joined room tile. Expected: preview sheet with name, about, member count, Join/Request button; "Peek inside" streams recent artifacts; tapping Join dispatches `requestJoinRoom` and the button reflects pending/joined.
14. **Room Home lane** — Steps: open a joined room → Home tab. Expected: artifact-grouped highlight lanes (same card module as feed) render ≥1 lane when the room has highlights.
15. **Room Library** — Steps: room → Library tab. Expected: list of room artifacts (articles/books/podcasts) as cards; tapping opens the artifact detail/reader.
16. **Room Discussions** — Steps: room → Discussions tab. Expected: list of kind:11 discussions; tapping opens discussion detail with OP + replies.
17. **Post a room discussion** — Steps: Discussions tab → new-discussion affordance → title + body → publish. Expected: composer dismisses; new discussion appears in the list.
18. **Room Chat (NIP-29)** — Steps: room with chat activity → Chat tab → type a message → send. Expected: message list renders; sent message appears optimistically then confirms; load-more works on scroll-to-top.
19. **Create a room** *(IA target)* — Steps: Rooms tab → explicit *+ / New room* affordance (toolbar/FAB) → **modal** create sheet → name + about + visibility → Create. Expected: create form is **behind an affordance, not inline**; on success advances to the invite (welcome) screen.
20. **Invite people to a room** — Steps: from create→welcome or room → invite affordance → search/paste an npub → select → Add. Expected: invite link card with copy/QR; selected invitee chips; Add dispatches `submitRoomInviteMembers`; toast confirms.

### Reading / Books
21. **Read a NIP-23 article + highlight it** — Steps: open an article → select text → choose *Highlight* (or *Highlight with note*). Expected: body renders; selection menu offers Highlight; publishing creates a kind:9802 (appears later in feed/room).
22. **Open a book detail** — Steps: from a book-sourced highlight or search → open book. Expected: cover + title/author + Passages list (reference highlights for that ISBN).
23. **Bookmark an article** — Steps: in reader or highlight detail → bookmark toggle. Expected: bookmark state flips; article appears under Bookmarks → Mine → Articles.

### OCR capture (physical book → highlight)
24. **Capture a highlight from a physical book via OCR** *(user's named flow; iOS-parity target — currently the biggest Android gap)* — Goal: photograph a printed page, OCR it, pick the quote and the book, publish. Steps: Highlights `+` (or room camera) → **camera** opens → photograph a book page → OCR runs and recognized text appears → drag-select the quote on the page → Next → metadata sheet → pick/scan the book (ISBN) → optional room + note → Publish. Expected: a kind:9802 highlight is published carrying the scanned page image + recognized alt text; it appears in the feed with the page image and quote. **Validation note:** an emulator has no real camera; validate with the emulator's virtual scene / an injected image and assert the OCR→draft→publish path, OR validate the iOS reference on simulator and assert Android reaches functional parity once camera+OCR land (Phase 4).
25. **Pick an existing book without camera** — Steps: capture flow → book picker → recents/search → select a book already shared in your rooms. Expected: recents grid is populated (requires `requestBookPickerRecents` on appear); selecting commits the book to the draft.
26. **Manual ISBN entry / scan** — Steps: book picker → barcode/Enter-ISBN → type a valid ISBN-13. Expected: ISBN validates (`normalizeIsbn`), preview ("Is this right?") shows cover/title/author, *Use* commits a pending book artifact.

### Profiles / social
27. **View a profile** — Steps: tap an author avatar/name anywhere. Expected: profile hero (banner/avatar/nip05/bio), follow button, tabs (Writing/Highlights/Communities) with content.
28. **Follow / unfollow** — Steps: on another user's profile → Follow. Expected: button toggles to Following; `toggleProfileFollow` dispatched.
29. **Edit own profile** — Steps: own profile → Edit → change display name / bio → save. Expected: fields editable; save dispatches `submitEditProfile`; profile reflects change.

### Composing / sharing / comments
30. **Comment on a highlight/article (NIP-22)** — Steps: open comments on an item → type → publish; then reply to a comment. Expected: top-level comment publishes and appears; **threaded reply** path works (`publishComment(parentEventId)`) — current Android gap.
31. **Like / bookmark a comment** — Steps: comment row → like, bookmark. Expected: counts/state update.
32. **Share an article/highlight to a community** — Steps: item → share → pick a joined community + note → share. Expected: share sheet lists joined communities; publishing dispatches `publishArtifactShare`/`shareHighlightRepost`; toast confirms.

### Search / discovery
33. **Search and open a result** — Steps: Search tab → type a query → submit. Expected: sectioned results (Highlights/Articles/Communities/People); tapping an article result opens the reader, a community result opens the room, a person opens the profile (current Android gap: people/community rows non-tappable).

### Settings / network
34. **Relay management** — Steps: Settings → Network → view relays (live status), add a relay, probe NIP-11. Expected: relay rows with state/RTT; add dispatches `upsertNetworkRelay`; NIP-11 probe populates relay info.
35. **Reconnect / wifi-only** — Steps: Network → Reconnect All; toggle Wi-Fi-only. Expected: reconnect dispatched, statuses refresh; toggle persists.
36. **Blossom media servers** — Steps: Settings → Media → add/remove/reorder a Blossom server. Expected: list mutates; `addBlossomServer`/`moveBlossomServers` dispatched.

### Podcast (parity-optional but present)
37. **Play a podcast + scrub** — Steps: open a podcast artifact → play → scrub. Expected: ExoPlayer plays; mini player appears; scrubber works.
38. **Create a podcast clip highlight** — Steps: podcast → clip affordance → set start/end → compose → publish. Expected: clip composer with range; publish dispatches `publishClipHighlight`. *(Android currently lacks transcript/waveform layers — partial.)*

> **Flow count: 38** (covers the user's three named basics — #6 feed, #12 open room, #24 OCR capture — plus the full breadth). Use #1, #6, #11, #12, #19, #24 as the **smoke set** every phase must pass.

---

## 3. Android Gap Audit

State legend: **WORKING** / **PARTIAL** / **BROKEN** / **MISSING**. Files are under `app/android/app/src/main/java/com/highlighter/app`.

### Shell & bridge — WORKING
| Area | Android file | State | Root-cause / note |
|---|---|---|---|
| Core bridge (listen + callback + dispatch) | `HighlighterViewModel.kt` (listenForUpdates `:69`, registerEventBridge in `bootstrap()` before login `:81/:218`, `onState`→StateFlow `:201`, `dispatch`→`app.dispatch` `:114`) | WORKING | Push-based StateFlow, recomposition on emit. EventBridge armed pre-login (the NMP wiring fix). This is the most robust layer — *not* the cause of any "nothing works". |
| Event delta bridge | `EventBridge.kt` | WORKING | NIP-46 `SignerConnected` → `refreshAppChrome` |
| Credential persistence | `SessionStore.kt` | WORKING | encrypted nsec restore on bootstrap |
| Auth/onboarding gate | `ui/RootScene.kt` (`:54-99`, overlays `:201-244`) | WORKING | mirrors `RootSceneView.swift` |
| 3-tab scaffold | `ui/MainScaffold.kt` (tabs `:54-59`, bar `:244-263`) | WORKING | HIGHLIGHTS/ROOMS/SEARCH match iOS `MainTabView` |
| Destination chrome | `ui/ScreenChrome.kt`, `MainActivity.kt` | WORKING | back-bar + providers (`LocalDispatch`, `LocalProfiles`) |

### Feed / Highlights — WORKING (timing/cap caveats)
| Area | Android file | State | Root-cause / note |
|---|---|---|---|
| Highlights tab wiring | `ui/MainScaffold.kt` `HighlightsTab` `:290-310` | WORKING | dispatches `OpenHomeFeed` on appear `:295`, `RefreshHomeFeed` on pull `:300`, renders `HomeFeedPanel(state.homeFeed)` `:307` |
| Feed rendering | `ui/home/HomeFeedPanel.kt` (`:36-174`) | PARTIAL | renders highlight + read rows + empty state. **`take(8)` cap `:68`** silently truncates; otherwise faithful |
| **"Feed shows nothing" diagnosis** | — | — | **Not a Compose wiring bug.** `OpenHomeFeed` fires correctly. If empty, cause is upstream: (a) relay/sync latency (status shows "Syncing/Connecting" `MainScaffold.kt:203`), (b) account genuinely has no followed-author highlights, or (c) the `take(8)` cap hiding paged data. Fix = ensure sync completes + remove/raise cap + show a loading state distinct from empty. |

### Rooms — PARTIAL (real bugs)
| Area | Android file | State | Root-cause / note |
|---|---|---|---|
| Rooms explorer | `ui/rooms/RoomExplorerPanel.kt` (`:184-241`, `OpenRoom` `:194`, Join `:233`) | WORKING | renders shelves; tile tap dispatches `OpenRoom(room.id)` |
| Room detail | `ui/rooms/RoomDetailPanel.kt` (`:48-161`) | WORKING | artifacts/highlights/discussions/chat/publish all present; opened as overlay when `roomDetail.groupId` non-blank (`RootScene.kt:229`) |
| **"Opening rooms does nothing" diagnosis** | `RootScene.kt:229-233` + `MainScaffold.kt:320` | BROKEN (effective) | Opening *does* set `roomDetail` and overlay `RoomDetailPanel`. BUT **`RoomsTab` onDispose dispatches `CloseRoom`** (`MainScaffold.kt:320`) instead of an explorer-close. Any recomposition/tab disposal collapses `roomDetail` and dismisses the just-opened room — producing the "opening rooms does nothing / it closes instantly" experience. **Fix:** change `:320` to not dispatch `CloseRoom` (use a no-op or explorer-specific close; confirm whether a `closeRoomExplorer` action exists, otherwise omit). |
| **Create-room placement (IA bug)** | `ui/rooms/CreateRoomPanel.kt` + `MainScaffold.kt:331-333` | BROKEN (IA) | **The form is rendered as the permanent first `item {}` of the Rooms `LazyColumn`, above the explorer** — exactly the user complaint. iOS shows a `+` toolbar button presenting `CreateRoomSheet` modally (`RoomExplorerView.swift:42-48,76`). **Fix:** remove the inline `CreateRoomPanel` item; add a `+`/FAB affordance presenting it as a `ModalBottomSheet` or `ScaffoldRoute`. |
| Room invite | `ui/rooms/RoomInvitePanel.kt` (`:48,89`) | PARTIAL | functional but shows raw hex pubkeys (no profile hydration). Cosmetic. |

### OCR Capture — PARTIAL → effectively MISSING for the named flow
| Area | Android file | State | Root-cause / note |
|---|---|---|---|
| Capture screen | `ui/capture/CapturePanel.kt` (only capture file, 294 lines) | PARTIAL | Implements: gallery photo pick → `UploadCapturePhoto` `:66-80`; artifact text search → `SearchBookPickerArtifacts` `:101`; publish `PublishCaptureHighlight`/`PublishCapturePicture` `:183-226`. |
| **Camera** | — | MISSING | No CameraX/intent anywhere; manifest has **no `CAMERA` permission** (`AndroidManifest.xml` only INTERNET + ACCESS_NETWORK_STATE). iOS = `CameraView.swift`, `BookScannerView.swift`. |
| **OCR** | — | MISSING | No ML Kit / on-device OCR. iOS = `OCRService.swift` + `OCRStructureReconstructor.swift`. Android always sends empty `alt` + manually-typed quote. |
| **ISBN entry/lookup/scan** | — | MISSING | Zero references to `RequestIsbnPreview`/`normalizeIsbn`/`isbnPreviews`. iOS = `ManualISBNEntryView`, `ISBNValidator`, `BookScannerView`. |
| **Book recents priming** | `CapturePanel.kt` | BROKEN | never dispatches `RequestBookPickerRecents`, so recents grid is empty on open (iOS does it on appear, `BookPicker.swift:58`). |

### Reader / Profile / Bookmarks / Settings / Podcast — WORKING
| Area | Android file | State | Note |
|---|---|---|---|
| Article reader | `ui/reader/ArticleReaderPanel.kt` | WORKING | renders + publishes highlights |
| Profile / edit | `ui/profile/ProfilePanel.kt`, `EditProfileScreen.kt` | WORKING | stats, follow, image uploads |
| Bookmarks / curation | `ui/bookmarks/BookmarkLibraryPanel.kt`, `CurationMenuSheet.kt` | WORKING | articles/collections/web/explore |
| Settings / network / media | `ui/SettingsScreen.kt`, `ui/settings/SettingsPanels.kt` | WORKING | relays w/ live status, blossom |
| Share composer | `ui/share/ShareComposerPanel.kt` | WORKING | host payload + `PublishUrlShare` |
| Feedback | `ui/feedback/FeedbackPanel.kt` | WORKING | threads + reply |
| What's New | `ui/whatsnew/WhatsNew.kt` | WORKING | state-driven dialog |
| Podcast | `ui/podcast/*` | PARTIAL | real ExoPlayer + transport + chapters; **no waveform/transcript layers** (intentionally deferred); clip path present |
| Auth screens | `ui/auth/*` | WORKING | login (nsec+NIP-46), create-account (live username), welcome, interests |

### Comments / Search — PARTIAL (cosmetic/feature gaps)
| Area | Android file | State | Note |
|---|---|---|---|
| Comments | `ui/comments/CommentsPanel.kt` (`:41,62,70`) | PARTIAL | top-level comment + like/bookmark work; **no threaded reply UI** (only the `null`-parent draft exposed though core supports `parentEventId`) |
| Search | `ui/search/SearchPanel.kt` | PARTIAL | query/recents/grouped results render; **profile & community result rows non-tappable** (article rows work) |

### Gap summary counts
- **WORKING:** shell/bridge (6), feed wiring, room explorer, room detail, reader, profile/edit, bookmarks/curation, settings/network/media, share composer, feedback, what's-new, auth (4 screens). ≈ **22 areas.**
- **PARTIAL:** HomeFeedPanel (cap), RoomInvitePanel (raw pubkeys), Capture (gallery-only), Podcast (no waveform/transcript), Comments (no replies), Search (non-tappable rows). ≈ **6 areas.**
- **BROKEN:** Create-room IA placement, RoomsTab `CloseRoom` teardown, Book-recents priming. ≈ **3 areas.**
- **MISSING:** Camera capture, OCR engine, ISBN entry/lookup/scan, CAMERA manifest permission. ≈ **4 items (one cohesive feature: OCR capture).**

> Mapped to the 38 flows: roughly **26 WORKING, 7 PARTIAL, 2 BROKEN (flows #12, #19), 3 MISSING (flows #24, #26, and the camera half of #25)**. The user's "complete disaster" impression is driven disproportionately by flows #12 and #19 (rooms feel broken) and the absence of #24 (the marquee OCR feature) — i.e. a few defects dominate perception.

---

## 4. UI / Information-Architecture problems & reorganization plan

### IA-1 — Create-room form dumped at top of Rooms list **(the headline bug)**
- **Now:** `MainScaffold.kt:331-333` renders `CreateRoomPanel` as the first item of the Rooms `LazyColumn`, so every visit to Rooms opens on a create-room form, pushing the explorer below the fold. This reads as "the default action of Rooms is to create one."
- **iOS reference:** `RoomExplorerView.swift` — Rooms opens directly on the explorer (hero + shelves). Create is a single top-bar-trailing `+` ("New room", `:42-48`) presenting `CreateRoomSheet` as a `.large` **modal sheet** (`:76`).
- **Fix:** (a) delete the inline `CreateRoomPanel` item; (b) add a `+`/FAB or top-bar action on the Rooms tab; (c) present `CreateRoomPanel` inside a `ModalBottomSheet` (or a `ScaffoldRoute.CreateRoom` full-screen route) gated by that affordance; (d) on success (`createRoom.createdGroupId` non-blank) route to the invite/welcome screen, mirroring iOS.

### IA-2 — Opening a room is fragile (teardown dismisses it)
- **Now:** `RoomsTab` onDispose dispatches `CloseRoom` (`MainScaffold.kt:320`), collapsing `roomDetail` whenever the tab is disposed (e.g. switching tabs after opening a room) and dismissing the room overlay.
- **iOS reference:** the explorer's lifecycle (`openRoomExplorer`/`refreshRoomExplorer`) is independent of an open room's lifecycle (`openRoom`/`closeRoom`, owned by `RoomHomeView`/`RoomPreviewSheet`). Closing the explorer must not close an open room.
- **Fix:** remove the `CloseRoom` from `RoomsTab` onDispose. Let `RoomDetailPanel`'s own lifecycle own `closeRoom` (it should dispatch `closeRoom` when the overlay/back is dismissed). Confirm room-open navigation is stable across recomposition.

### IA-3 — Primary actions should live in toolbar/FAB, not inline forms
- Audit every tab/screen for inline create/compose forms that should be affordance-gated, matching iOS:
  - Highlights: capture is already a `+` FAB (`MainScaffold.kt:266`) — keep.
  - Rooms: create → toolbar/FAB modal (IA-1).
  - Room Home: per-tab contextual actions (Home→camera capture into room; Library→suggest; Discussions→new discussion; always→invite). iOS uses contextual top-bar buttons (`RoomHomeView.swift`). Android `RoomDetailPanel` should expose the same per-tab affordances rather than always-visible forms.
  - Discussions composer, invite: present as sheets/routes, not inline.

### IA-4 — Tappability & hydration polish (parity correctness)
- Search profile/community rows should navigate (open profile / open room) like iOS (`SearchView.swift`).
- Room invite candidates should hydrate display names/avatars via `requestProfile` instead of showing raw hex (`RoomInvitePanel.kt:48,89`), matching `RoomInviteView.swift`.
- Comments should expose threaded replies (`publishComment(parentEventId)`), matching `ThreadView`/`CommentRow`.

### IA-5 — Feed truncation & loading clarity
- Remove or raise the `HomeFeedPanel.kt:68` `take(8)` cap so the feed isn't silently clipped; show a distinct loading state vs the "No highlights yet" empty state so a syncing feed isn't misread as broken (the root of the "no feed" perception).

### Target navigation hierarchy (mirrors iOS)
```
RootScene
├─ (logged out) Welcome → Login / Create account → Interests
└─ (logged in) MainScaffold
   ├─ Tab: Highlights → HomeFeedPanel        [+ FAB → Capture route]
   │     → Highlight detail → source reader (article/book/web)
   ├─ Tab: Rooms → RoomExplorerPanel          [+ toolbar/FAB → CreateRoom sheet]
   │     → Room preview sheet → Join
   │     → Room detail (overlay): Home / Library / Discussions / Chat
   │           Home camera→Capture(into room); Library→Suggest; Discussions→New; →Invite sheet
   └─ Tab: Search → SearchPanel → results → reader/room/profile
   Overlays (RootScene): Profile, ArticleReader, RoomDetail, RoomInvite, Comments
   Routes (ScaffoldRoute): Settings, Capture, Bookmarks, Feedback, Podcast
   Top bar: avatar → Profile; menu → Bookmarks / Settings / Log out
```

---

## 5. Validation Harness Plan (run the app for real)

Validation is mandatory: a phase is "done" only when a validator agent (Haiku) drives the **running** app and observes the **Expected** outcome of each flow via screenshots.

### 5.1 Confirmed environment facts (verified on this machine)
- Debug APK: `~/Builds/highlighter-debug.apk` (54 MB, **arm64-v8a only**).
- `adb`: `/opt/homebrew/bin/adb` and `~/Library/Android/sdk/platform-tools/adb`.
- Emulator: `~/Library/Android/sdk/emulator/emulator`.
- AVD present: **`HighlighterTest`** = `system-images/android-34/google_apis/arm64-v8a` (ABI matches the APK — required, since the app ships arm64-only native `libhighlighter_core.so`).
- Maestro: `~/.maestro/bin/maestro` (available).
- iOS reference: iPhone 16 simulator **already booted** (`xcrun simctl`), iOS 26.x images available.

### 5.2 Android: launch + drive
1. Boot the matching AVD (must be arm64-v8a):
   `~/Library/Android/sdk/emulator/emulator -avd HighlighterTest -no-snapshot -gpu swiftshader_indirect &`
   then `adb wait-for-device; adb shell getprop sys.boot_completed` until `1`.
2. Install: `adb install -r ~/Builds/highlighter-debug.apk`.
3. Launch: `adb shell monkey -p com.highlighter.app -c android.intent.category.LAUNCHER 1` (confirm package id from the manifest/applicationId before use).
4. Drive UI + capture:
   - **Primary (recommended): Maestro.** It's installed, YAML flows are readable, it does view-hierarchy assertions + `takeScreenshot`, and tolerates async UI with `extendedWaitUntil`. One `.yaml` flow per numbered flow in §2 (e.g. `06-highlights-feed.yaml`, `12-open-room.yaml`, `19-create-room.yaml`). Run: `maestro test flows/06-highlights-feed.yaml`.
   - **Fallback: adb + uiautomator.** `adb exec-out uiautomator dump /dev/tty` for the view tree, `adb shell input tap/text/swipe` to drive, `adb exec-out screencap -p > shot.png` for screenshots. Use when a flow needs precise coordinate taps or Maestro can't see a Compose node.
   - Compose testTags: where Maestro/uiautomator can't find a node by text, add `Modifier.testTag(...)` (this is the one allowed UI-code touch for testability, done as part of the relevant phase, not this plan).
5. Logs/triage: `adb logcat -s Highlighter:* AndroidRuntime:E` during each flow to catch core errors/crashes; correlate "feed empty" with sync status lines.

### 5.3 iOS reference: launch + drive (for parity baselines)
- Simulator already booted. Build/run: `xcodebuild -workspace app/ios/Highlighter.xcworkspace -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16 ci'` then install to the booted sim.
- Screenshot: `xcrun simctl io booted screenshot ref.png`. Use these as the visual ground truth for each flow's Expected outcome (capture the iOS screen for flows #6, #11, #12, #19, #24 first to define "correct").

### 5.4 Getting REAL Nostr data in
- **Auth via nsec (preferred for automation):** use a dedicated test `nsec1…` (the same paste path `LoginView.swift` / Android `LoginScreen.kt` use; `signInNsec(persist:true)`). This sidesteps the account-creation hang (Known Issues #1). Provision a test account that already **follows several active highlighters and has joined ≥2 rooms**, so flows #6/#11/#12/#14 have non-empty data. Store the nsec out-of-band (e.g. an env var the validator reads), never in the repo.
- **Relays:** rely on the app's default relay set; verify connectivity in Settings → Network (status should reach "Connected"). If the feed is empty, first confirm relays connected and the test account's follow graph is populated before classifying it a bug.
- **Seed content for capture (#24-26):** pre-share a couple of book artifacts into a test room from iOS so Android book-recents/search has data; for OCR, use a fixed page image (emulator virtual-scene or injected file) since the emulator lacks a real camera.

### 5.5 Blockers / caveats
- **Emulator has no camera/OCR-grade input** → flow #24 must validate via injected image or be cross-checked against the iOS simulator reference; full on-device OCR proof needs a physical Android device or camera-emulation.
- **arm64-only APK** → only arm64 system images work; an x86_64 AVD will fail to load the native lib. (Apple-Silicon host is fine.)
- **Account-creation hang** (Known Issues #1) → avoid signup in automation; use nsec login. If a phase touches signup, treat a >30s "Creating…" spinner as the known core defect, not an Android regression.
- **App Links** (Known Issues #4) → deep-link flows need `/.well-known/assetlinks.json` served; until then validate deep links via `adb shell am start -a android.intent.action.VIEW -d "<uri>"` directly.
- **NIP-05 endpoint 404** (Known Issues #3) → username-availability checks may not fully resolve; don't fail signup validation solely on that.
- **Flaky core test** (Known Issues #2) → ignore single intermittent `cargo test --lib` failures when gating on the core.

---

## 6. Prioritized Implementation Roadmap

Each phase is sized for one coding agent (Sonnet) to implement, then a validator (Haiku) to prove via §5 against the listed flows. Phases are dependency-ordered; "Basics" phases come first.

### Phase 0 — Validation harness bring-up (no app code) — UNBLOCKS EVERYTHING
- Boot `HighlighterTest` AVD, install the existing APK, log in with the test nsec (§5.4), confirm "Connected".
- Capture iOS reference screenshots for the smoke set (#1,#6,#11,#12,#19,#24).
- Author Maestro flows for the smoke set; establish the screenshot baseline.
- **Exit:** smoke flows run end-to-end on Android (even where they currently fail), producing screenshots that document the *current* broken state. Gate: harness reproducibly drives the app.

### Phase 1 — Rooms basics (UNBLOCKS "open a room") — flows #12, #11
- Fix IA-2: remove `CloseRoom` from `RoomsTab` onDispose (`MainScaffold.kt:320`); ensure `RoomDetailPanel` owns its own `closeRoom` on dismiss; verify room-open survives recomposition/tab switches.
- **Validate:** #12 (open a room → room screen stays open), #11 (explorer renders shelves). 
- **Exit:** tapping a room opens and stays on the room.

### Phase 2 — Rooms IA: create-room relocation (UNBLOCKS clean Rooms) — flows #19, #11, #20
- Fix IA-1: delete inline `CreateRoomPanel` item; add `+`/FAB toolbar affordance presenting it as a modal sheet/route; route to invite on success.
- Wire invite hydration (IA-4) if cheap; otherwise keep raw and defer.
- **Validate:** #11 (Rooms opens on explorer, no top form), #19 (create behind affordance, modal), #20 (invite).
- **Exit:** Rooms tab opens on content; create-room is modal; user complaint resolved.

### Phase 3 — Feed clarity (UNBLOCKS "see the feed") — flows #6, #7, #8, #10
- Fix IA-5: remove/raise `take(8)`; distinct loading vs empty state; confirm `OpenHomeFeed` + sync produce cards with the test account.
- Verify highlight-card and reading-card rendering parity (resource header, quote/page image, social badge) vs iOS screenshots.
- **Validate:** #6 (feed populates ≥1 card), #7 (refresh), #8/#10 (open detail/article).
- **Exit:** feed reliably shows real highlights; no silent truncation.

> Phases 1–3 close the three user-named "basics" except OCR (Phase 4).

### Phase 4 — OCR capture (the marquee gap) — flows #24, #25, #26
Largest phase; may be split into 4a/4b.
- **4a — Book picker + ISBN + recents:** dispatch `RequestBookPickerRecents` on Capture appear; add manual ISBN entry + `RequestIsbnPreview`/`normalizeIsbn` + the "Is this right?" preview; barcode scan (CameraX + ML Kit Barcode). Validate #25, #26.
- **4b — Camera + OCR:** add `CAMERA` permission to the manifest; CameraX capture (or document-scan equivalent); on-device OCR via **ML Kit Text Recognition**; page segmentation + structure→markdown + alt text; drag-select on the page; build `HighlightDraft` and publish via `publishCaptureHighlight`/`publishCapturePicture`. Validate #24 (against injected image + iOS reference).
- **Exit:** a highlight can be captured from a (real or injected) book page with OCR'd quote + page image and appears in the feed.

### Phase 5 — Social/compose polish — flows #30, #31, #32, #33, #27-#29
- Comments threaded replies (`publishComment(parentEventId)`), `CommentsPanel.kt`.
- Search profile/community rows tappable (`SearchPanel.kt`).
- Re-verify profile view/follow/edit and share-to-community parity.
- **Validate:** #30/#31 (comments+replies), #32 (share), #33 (search nav), #27-#29 (profile).
- **Exit:** social actions reach iOS parity.

### Phase 6 — Reading/books + settings parity — flows #21, #22, #23, #34, #35, #36
- Confirm article highlight publish, book detail/passages, bookmark, relay/media settings all match iOS; fix any deltas surfaced by validation.
- **Exit:** reading + settings fully parity-validated.

### Phase 7 — Podcast depth (parity-optional) — flows #37, #38
- Add transcript + waveform layers and clip composer parity if prioritized; otherwise document as accepted partial.
- **Exit:** podcast reaches agreed parity bar.

### Cross-cutting (every phase)
- Add `Modifier.testTag(...)` to nodes the harness can't reach by text (incremental, per-phase).
- Each phase: Sonnet implements → Haiku runs the smoke set + the phase's listed flows → screenshots vs iOS reference → fix until green.

### Top 5 most critical fixes (do first)
1. **Remove `CloseRoom` from `RoomsTab` onDispose** (`MainScaffold.kt:320`) — restores "open a room" (flow #12). *Tiny, highest impact.*
2. **Relocate create-room to a modal affordance** (`MainScaffold.kt:331-333` + `CreateRoomPanel.kt`) — fixes the headline IA complaint (flow #19/#11).
3. **Fix feed truncation/loading state** (`HomeFeedPanel.kt:68`) + confirm sync — restores "see the feed" perception (flow #6).
4. **Build OCR capture** (manifest CAMERA + CameraX + ML Kit OCR + ISBN/recents) — the marquee missing feature (flow #24); prime `RequestBookPickerRecents`.
5. **Comments threaded replies + search row tappability** — closes the most visible remaining parity gaps (flows #30, #33).

---

## Appendix — quick file cross-reference (iOS ↔ Android)
| Concern | iOS | Android |
|---|---|---|
| Root gate | `Navigation/RootSceneView.swift` | `ui/RootScene.kt` |
| Tab shell | `Navigation/MainTabView.swift` | `ui/MainScaffold.kt` |
| Home feed | `Features/Highlights/HighlightsTabView.swift` + `HighlightFeedCardView.swift` | `ui/home/HomeFeedPanel.kt` |
| Rooms explorer | `Features/Communities/RoomExplorerView.swift` | `ui/rooms/RoomExplorerPanel.kt` |
| Room detail | `Features/Communities/RoomHomeView.swift` (+Chat/Discussion/Lanes) | `ui/rooms/RoomDetailPanel.kt` |
| Create room | `Features/Communities/CreateRoom/CreateRoomSheet.swift` | `ui/rooms/CreateRoomPanel.kt` |
| Invite | `Features/Communities/CreateRoom/RoomInviteView.swift` | `ui/rooms/RoomInvitePanel.kt` |
| Capture/OCR | `Features/Capture/*` (15 files) | `ui/capture/CapturePanel.kt` (1 file — large gap) |
| Article reader | `Features/Article/ArticleReaderView.swift` | `ui/reader/ArticleReaderPanel.kt` |
| Profile | `Features/Profile/ProfileView.swift` | `ui/profile/ProfilePanel.kt` |
| Comments | `Features/Comments/*` | `ui/comments/CommentsPanel.kt` |
| Search | `Features/Search/SearchView.swift` | `ui/search/SearchPanel.kt` |
| Settings/Network | `Features/Settings/*` | `ui/SettingsScreen.kt` + `ui/settings/SettingsPanels.kt` |
| Bridge | `Core/HighlighterStore.swift`, `Core/EventBridge.swift` | `HighlighterViewModel.kt`, `EventBridge.kt`, `SessionStore.kt` |
| Generated binding | `Core/Generated/highlighter_core.swift` | `build/generated/source/uniffi/.../highlighter_core.kt` |
