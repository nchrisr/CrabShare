---
name: peer-identity-and-routing
title: Peer identity and routing
involvement: teach-me
owns:
  - 'src/main.rs'
  - 'src/peer.rs'
watch:
  - '\.route\(|on_upgrade\(|fallback_service\('
  - 'Uuid::new_v4\(\)'
  - '\.to\b.*\.get(_mut)?\(|\.get(_mut)?\(&?[a-z_]*\.to\b'
---

# Peer identity and routing

**Involvement:** teach-me. A mistake here lets one peer speak as another, or sends a message to a peer it wasn't meant for. Tier 3 item 2.
**The question this card asks of every change:** who decided this peer ID or this recipient, the server or the client?

## Invariants (always true)
- The server assigns every peer ID (`Uuid::new_v4` in the `/ws` handler). A client never chooses its own.
- When relaying, `from` is overwritten with the sender's real peer ID, whatever the client sent.
- A message goes to `to` only if `to` is in the sender's own subnet group. An unknown `to` is dropped and logged by type.
- `welcome` is the first message every peer receives, queued before the peer is visible to anyone else.

## Tripwires (changes that pull you in, even from a trusted area)
- A new route or entry point.
- A new place that creates a peer ID.
- Looking up a recipient by a client-supplied `to`.

## How it breaks
- No incidents yet. Routing of `offer`, `answer` and `ice_candidate` isn't built (M2).

## Checks
- **Programs:** unit tests in `src/peer.rs`.
- **Reviewer:** for each new message path: can a client make it claim another `from`? Can it reach a peer in another subnet group? What happens with an unknown or stale `to`?

## Briefing
How a message travels from one socket to another, which fields the server trusts and which it overwrites, and 2 or 3 ways a hostile client could try to break it.
