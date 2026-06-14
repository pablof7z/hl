---
title: Loop Command
slug: loop-command
topic: loop-command
summary: The /loop command parses input by first checking for a leading interval token matching ^\d+[smhd]$, then a trailing 'every <time>' clause, and falls back to dyn
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

# Loop Command

## Input Parsing

The /loop command parses input by first checking for a leading interval token matching ^\d+[smhd]$, then a trailing 'every <time>' clause, and falls back to dynamic self-paced mode if neither is found. If /loop input parses to an empty prompt, the system shows usage information '/loop [interval] <prompt>' and stops. <!-- [^16ac1-4] -->

When /loop detects an interval ≥60 minutes or daily phrasing, it offers the user a choice between a cloud schedule (using the schedule skill) or a session-only loop before proceeding. <!-- [^16ac1-5] -->

If the user picks 'Cloud schedule', the system invokes the schedule skill with the original input verbatim and stops without creating a local CronCreate job or executing the prompt immediately. If the user picks 'This session only' for daily-phrased input with no parsed interval (rule 3), the system explains that a daily-cadence loop won't fire before the session closes and suggests picking Cloud schedule or re-running /loop with a shorter explicit interval. <!-- [^16ac1-6] -->

## Fixed-Interval Mode

Fixed-interval /loop scheduling converts intervals to cron expressions (e.g. Nm≤59 to */N * * * *, Nh≤23 to 0 */N * * *, Nd to 0 0 */N * * *, Ns rounded up to minutes), rounding intervals that don't cleanly divide their unit and informing the user. <!-- [^16ac1-7] -->

CronCreate schedules for fixed-interval loops use recurring:true, and the confirmation message includes the job ID, cron expression, human-readable cadence, 7-day auto-expiry, and how to cancel via CronDelete. If the cloud-schedule offer was not shown (neither trigger condition applied), the CronCreate confirmation ends with the italicized line: _Runs until you close this session · For durable cloud-based loops, use /schedule_. <!-- [^16ac1-8] -->

After scheduling a fixed-interval loop, the system immediately executes the parsed prompt (invoking slash commands via the Skill tool) rather than waiting for the first cron fire. <!-- [^16ac1-9] -->

## Dynamic Mode

In dynamic mode (no interval), the system self-paces by running the prompt, arming a Monitor with persistent:true if the next run is gated on an event, and calling ScheduleWakeup with a fallback heartbeat of 1200–1800 seconds when a Monitor is active. ScheduleWakeup passes the prompt as the full original /loop input prefixed with '/loop ' so the next firing re-enters the skill and continues the loop. <!-- [^16ac1-10] -->

To stop a dynamic loop, the system omits the ScheduleWakeup call, stops any armed Monitor, and sends a one-line outcome via PushNotification (unless the user explicitly told it to stop). <!-- [^16ac1-11] -->
