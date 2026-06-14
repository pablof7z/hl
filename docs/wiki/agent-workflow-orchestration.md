---
title: Agent Workflow Orchestration
slug: agent-workflow-orchestration
topic: agent-system
summary: All work is delegated to agents/workflows â never done by the orchestrator directly â with Haiku for simple tasks (builds, simulators), Sonnet for coding, a
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-13
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Agent Workflow Orchestration

## Agent & Workflow Delegation

All work is delegated to agents/workflows — never done by the orchestrator directly — with Haiku for simple tasks (builds, simulators), Sonnet for coding, and Opus for planning/reviewing. <!-- [^84748-1] -->

Sonnet agents implement each roadmap phase and Haiku agents run the emulator, install the APK, drive the UI, and validate each flow against real data with screenshots; nothing is marked done until an agent has actually seen real highlights/rooms/data load; Opus reviews diffs and validation evidence before parity is declared. <!-- [^84748-2] -->
