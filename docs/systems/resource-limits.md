---
name: resource-limits
title: Resource limits
involvement: keep-me-posted
owns:
  - 'src/main.rs'
watch:
  - 'max_(message|frame)_size\('
  - 'const (MAX|DEFAULT_MAX)_\w+|Duration::from_\w+\('
---

# Resource limits

**Involvement:** keep-me-posted. Limits are tuning, until a public instance makes them a defence (D3).
**The question this card asks of every change:** what does one client get to use up, and how does the server get it back?

## Invariants (always true)
- Incoming WebSocket messages and frames are capped at `MAX_INCOMING_MESSAGE_BYTES` (16 KB), set before the upgrade.
- Each inbox holds at most `DEFAULT_MAX_MESSAGE_COUNT` messages.
- Every wait on a client has an end: a timeout, a closed socket, or a dropped connection.

## Tripwires (changes that pull you in, even from a trusted area)
- A changed size, count or timeout.

## How it breaks
- The WebSocket defaults allowed 64 MiB messages, fully buffered before being ignored, so a few connections could exhaust memory. Fixed by the 16 KB limits.

## Checks
- **Programs:** none yet.
- **Reviewer:** what is the worst case per connection, and times a thousand connections?

## Briefing
Each limit changed, its old and new value, and the worst case it allows.
