---
title: Android Comment Threads
slug: android-comment-threads
topic: nmp-app
summary: CommentsPanel builds a threaded tree from topLevelEventIds and childLinks, renders top-level nodes with indented reply previews, and supports a reply affordance
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

# Android Comment Threads

## Comment Threading

CommentsPanel builds a threaded tree from topLevelEventIds and childLinks, renders top-level nodes with indented reply previews, and supports a reply affordance with a 'Replying to @name' banner (previously: CommentsPanel was rewritten to support threaded replies using a buildCommentTree() from topLevelEventIds/childLinks, with a replyingToEventId state, a ReplyContextBanner, and per-row RequestProfile hydration). PublishComment carries the optional parentEventId so the core publishes the reply to the correct parent thread. The HighlightDetailScreen's Comment action dispatches OpenComments(rootTagName='e', rootTagValue=highlight.eventId, rootKind=9802u) to open the threaded CommentsPanel overlay. Tapping a feed highlight card opens a HighlightDetailScreen with the quote, tappable author→profile, and a Comment action that opens the threaded CommentsPanel, instead of directly opening the article reader. Comments entry from the feed requires the new HighlightDetailScreen; OpenComments is only dispatched from inside rooms (RoomDetailPanel) and highlight detail, not from feed cards directly. Search profile/community rows are non-tappable.

<!-- citations: [^84748-68] [^84748-84] [^84748-95] [^84748-104] [^84748-120] [^84748-128] [^84748-143] [^84748-153] [^84748-168] [^84748-182] [^84748-194] -->
