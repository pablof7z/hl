---
title: Disk Cleanup
slug: disk-cleanup
topic: disk-cleanup
summary: The disk space monitoring loop triggers cleanup when free space drops below 5GB
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-13
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:16ac1219-405e-4d37-bcba-f2ad417a7e1e
---

# Disk Cleanup

## Disk Cleanup

The disk space monitoring loop triggers cleanup when free space drops below 5GB. The target is at least 80GB of free space after cleanup. Cleanup avoids deleting anything important and prioritizes build artifact directories in Library, ~/src, and ~/Work. <!-- [^16ac1-1] -->

When disk space approaches critical levels (near 5GB) but hasn't crossed the threshold, the system proactively cleans Rust target/ directories from completed (unlocked) agent worktrees, avoiding active (locked) worktrees. <!-- [^16ac1-2] -->

## Safety Classification

Rust target/ directories are safe to delete because cargo build regenerates them. Xcode build folders and Swift package caches (e.g. ~/Library/Caches/org.swift.swiftpm) are safe to delete. iOS DeviceSupport older versions and signed app archives should be kept unless already uploaded. LoRA training model directories (e.g. ~/src/lora-training/fused-test) are likely important and should not be deleted as build artifacts. <!-- [^16ac1-3] -->
