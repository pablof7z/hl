---
title: Account Creation
slug: account-creation
topic: nmp-app
summary: Account creation no longer bricks when the NIP-05 availability check fails; a failed check lets the user proceed (the claim is skipped)
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-13
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Account Creation

## Account Creation

Account creation no longer bricks when the NIP-05 availability check fails; a failed check lets the user proceed (the claim is skipped). Account creation has a 30-second deadline and HTTP timeouts to prevent indefinite hangs on dead networks. Validation login must use an nsec (not signup) to avoid the known account-creation hang bug.

<!-- citations: [^0c7b6-100] [^0c7b6-116] [^0c7b6-158] [^0c7b6-171] [^84748-11] -->
