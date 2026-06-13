---
title: Build System Baseline Refresh
slug: build-system-baseline-refresh
topic: build-system
summary: File-size baseline violations caused by master's own drift are resolved by a separate post-merge baseline-refresh PR, following the precedent of PR #1196 for #1
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
---

# Build System Baseline Refresh

## Baseline Refresh Process

File-size baseline violations caused by master's own drift are resolved by a separate post-merge baseline-refresh PR, following the precedent of PR #1196 for #1192; the PR author does not raise the baseline numbers. <!-- [^f54b4-25] -->
