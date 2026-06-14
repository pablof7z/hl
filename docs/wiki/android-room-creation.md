---
title: Android Room Creation
slug: android-room-creation
topic: nmp-app
summary: CreateRoomPanel is removed from the Rooms LazyColumn and replaced with a FloatingActionButton that opens a ModalBottomSheet (CreateRoomSheet) containing the for
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-14
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Room Creation

## CreateRoomSheet Access

CreateRoomPanel is removed from the Rooms LazyColumn and replaced with a FloatingActionButton that opens a ModalBottomSheet (CreateRoomSheet) containing the form, mirroring iOS. Upon successful room creation (createRoom.createdGroupId is non-null and non-blank), the modal automatically dispatches OpenRoomInvite, clears the result, and dismisses, mirroring iOS CreateRoomSheet routing.

<!-- citations: [^84748-18] [^84748-19] [^84748-31] [^84748-112] [^84748-171] -->
## RoomsTab Lifecycle

The RoomsTab onDispose no longer dispatches CloseRoom, so rooms stay open across tab switches and recompositions. joinedRoomIds in RoomsTab is memoized via derivedStateOf to avoid allocating a new Set on every recomposition. PullToRefreshBox gesture callbacks (onRefresh = RefreshHomeFeed / RefreshRoomExplorer) remain in place as swipe-down gestures, not buttons.

<!-- citations: [^84748-133] [^84748-30] [^84748-54] [^84748-76] [^84748-113] [^84748-155] [^84748-172] [^84748-198] -->
## Test Tags

Test tags added for automation: `create_room_fab`, `room_explorer_list`, `feed_loading`, `feed_item_list`, `feed_highlight_card`, `feed_reading_card`, `card_cover`, `card_author`, `room_tile_cover`, `room_tile_name`, `room_detail_name`, `room_tab_home`, `room_tab_library`, `room_tab_discussions`, `room_tab_chat`, `room_new_discussion_fab`.

<!-- citations: [^84748-32] [^84748-60] -->
## Room Tile Display

Room tiles display room.name with a fallback of room.id.take(8)+'…' (not full hex), show room covers via RemoteImage with CoverShape, and display member subtitles (member count or 'Open room') instead of raw hex IDs.

<!-- citations: [^84748-42] [^84748-55] [^84748-77] [^84748-98] [^84748-114] [^84748-156] [^84748-173] -->
## Room Detail Panel

The room detail screen is a full-screen Scaffold with the room name in the TopAppBar (resolved from state.chrome.joinedCommunities), pill tabs (Home/Library/Discussions/Chat), and a discussion composer behind a FAB/IconButton modal, not inline forms. The Chat tab in RoomDetailPanel is only shown when chatMessageCount > 0, matching iOS's hasChatActivity guard. The discussion composer in RoomDetailPanel is a ModalBottomSheet opened by a '+' IconButton in the TopAppBar or a FAB on the Discussions tab; it auto-dismisses when lastPublishedDiscussionId changes. Room detail dispatches RequestProfile per author with deduplication (checks profiles.profileFor(pubkey) == null before dispatching), LoadMoreRoomChat on scroll-to-top, PublishRoomChatMessage on send, and PublishRoomDiscussion via a modal composer. CloseRoom is dispatched only from the back action.

<!-- citations: [^84748-61] [^84748-78] [^84748-99] [^84748-115] [^84748-157] [^84748-174] -->
## Verification

Flow #12 (open a room and it stays open) is verified: tapping Open on a room tile opens the room detail screen, which persists (no bounce-back), and pressing back returns cleanly to the explorer. The create-room modal (FAB → ModalBottomSheet) is verified: the Rooms tab opens the explorer (not an inline form), the FAB opens a bottom-sheet modal with Name/About/Public-Private/Open-Closed fields, and it dismisses cleanly. <!-- [^84748-62] -->

## SearchPanel Navigation

SearchPanel person and community rows are tappable, with onClick handlers dispatching OpenProfile(pubkey) and OpenRoom(community.id) respectively. <!-- [^84748-186] -->
