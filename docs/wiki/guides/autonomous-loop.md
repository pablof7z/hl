---
title: Autonomous Loop
slug: autonomous-loop
topic: autonomous-loop
summary: When `/loop` is invoked with no prompt or interval, the system runs the autonomous check immediately and then self-paces the next iteration via ScheduleWakeup.
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

# Autonomous Loop

## Invocation

When `/loop` is invoked with no prompt or interval, the system runs the autonomous check immediately and then self-paces the next iteration via ScheduleWakeup. <!-- [^9ae03-f42e2] -->

## Event Monitoring

If the next tick is gated on an external event (CI finishing, a PR comment, a log line) and no Monitor is already running, a persistent Monitor should be armed to wake the loop immediately on events rather than waiting for the ScheduleWakeup deadline. When a Monitor is armed, the fallback heartbeat `delaySeconds` should be between 1200–1800 seconds. <!-- [^9ae03-c7936] -->

## Self-Pacing with ScheduleWakeup

Before calling ScheduleWakeup, the system should briefly confirm that autonomous mode is active, that the check ran, whether a Monitor is the primary wake signal, and what fallback delay is being picked. ScheduleWakeup's `prompt` parameter should use the literal sentinel string `<<autonomous-loop-dynamic>>` in dynamic-pacing mode, not the full instructions; the expansion is handled automatically. <!-- [^9ae03-5dcfa] -->

## Termination

To stop the autonomous loop, omit the ScheduleWakeup call and TaskStop any armed Monitor; optionally send a one-line outcome via PushNotification before stopping. <!-- [^9ae03-0b62f] -->
