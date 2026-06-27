---
title: Bookmark Sets
slug: bookmark-sets
topic: bookmark-sets
summary: Bookmark set edit actions (rename, delete) are gated to sets owned by the active user
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-26
updated: 2026-06-26
verified: 2026-06-26
compiled-from: conversation
sources:
  - session:9ae03596-fa74-4208-88c6-a90bd3b176e4
---

# Bookmark Sets

## Edit Actions

Bookmark set edit actions (rename, delete) are gated to sets owned by the active user. The UI shows affordances only when `record.kind == 30004 && record.isOwned(by: app.currentUser?.pubkey)`. The reducer enforces the same gate via `find_owned_curation_set`, returning no-op if the set's pubkey does not match the active account. <!-- [^9ae03-d8073] -->

## Share URLs

Bookmark set share URLs use the `/note/` path prefix in the format `https://highlighter.com/note/<naddr>`, matching the web app's naddr routing for all link types rather than `/a/`. <!-- [^9ae03-9d1e5] -->
