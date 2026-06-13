# Highlighter Android — Maestro Test Flows

## Prerequisites

- Maestro CLI installed (`brew install mobile-dev-inc/tap/maestro` or https://maestro.mobile.dev)
- Debug APK installed on a connected device / emulator
- A test nsec stored somewhere outside the repo (e.g. `~/Builds/test-account.txt`)

## Running flows

```bash
# Single flow (login must run first on a fresh install):
MAESTRO_NSEC=$(grep -o 'nsec1[a-z0-9]*' ~/Builds/test-account.txt | head -1) \
  maestro test app/android/maestro/00-login.yaml

# Full suite in order:
MAESTRO_NSEC=$(grep -o 'nsec1[a-z0-9]*' ~/Builds/test-account.txt | head -1) \
  maestro test app/android/maestro/
```

`00-login.yaml` **must run first** on a fresh install to establish the
session.  Subsequent flows use `clearState: false` and rely on the persisted
session credential, so they can be run independently once the session exists.

## Flow inventory

| File | Purpose |
|------|---------|
| `00-login.yaml` | Launch, sign in with nsec (`${MAESTRO_NSEC}`), handle optional onboarding + What's New dialog, assert Highlights screen |
| `06-feed.yaml` | Assert feed populates with at least one highlight card within 30 s |
| `08-highlight-detail.yaml` | Tap first feed card, assert detail screen, author byline, and comment button |
| `30-comments.yaml` | Open highlight detail, tap comment button, assert composer; optionally tap Reply and assert "Replying" banner |
| `11-rooms-explorer.yaml` | Navigate to Rooms tab, assert FAB + explorer list + at least one room tile |
| `12-open-room.yaml` | Tap first room tile name, assert room detail name and Home tab pill |
| `19-create-room.yaml` | Tap create-room FAB, assert "Create room" / "NIP-29" modal sheet |
| `33-search-nav.yaml` | Search "nostr", wait for person row, tap it, assert Profile overlay |

## Secret handling

The nsec is **never** committed to the repository.  Flows reference it as
`${MAESTRO_NSEC}` — Maestro injects environment variables at runtime.
