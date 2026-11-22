---
name: frontend
title: Frontend
involvement: trust
owns:
  - 'static/*'
  - 'landing/*'
watch:
  - '\.innerHTML *=|insertAdjacentHTML\(|document\.write\(|\beval\(|new Function\('
---

# Frontend

**Involvement:** trust. Plain HTML, CSS and JavaScript; the demo shows whether it works.
**The question this card asks of every change:** does anything a peer sends end up as HTML or code?

## Invariants (always true)
- Text from the server or another peer goes into the page as text (`textContent`), never as HTML.
- No external scripts, fonts or trackers; the page loads only from the CrabShare server.
- The peer list matches the server's `welcome`, `peer_joined` and `peer_left` messages.

## Tripwires (changes that pull you in, even from a trusted area)
- Writing HTML from a string, or running a string as code.

## How it breaks
- No incidents yet.

## Checks
- **Programs:** none yet.
- **Reviewer:** where does each string on the page come from?

## Briefing
The demo: two tabs, a join and a leave.
