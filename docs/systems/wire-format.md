---
name: wire-format
title: Wire format
involvement: teach-me
owns:
  - 'src/messages.rs'
  - 'static/app.js'
watch:
  - '#\[serde\('
  - 'serde_json::from_(str|slice)\(|JSON\.parse\('
---

# Wire format

**Involvement:** teach-me. A change existing clients notice breaks every open tab and any native client (D8). Tier 3 item 4.
**The question this card asks of every change:** would a client built before this change still understand the server, and the other way round?

## Invariants (always true)
- `SignalingMessage` is JSON, tagged with `#[serde(tag = "type", rename_all = "snake_case")]` (D8).
- Adding a variant or an optional field is compatible. Renaming or removing one, or changing a type, isn't.
- Unknown or malformed client messages are logged by type and skipped; they never panic the server.

## Tripwires (changes that pull you in, even from a trusted area)
- Any serde attribute change.
- A new place that parses client JSON, on either side.

## How it breaks
- No incidents yet.

## Checks
- **Programs:** none yet. A round-trip test per variant would pin the JSON shape.
- **Reviewer:** show the JSON before and after for each changed variant. What does an old client do with the new shape?

## Briefing
The JSON of each new or changed message, who sends it, and what an older client does with it.
