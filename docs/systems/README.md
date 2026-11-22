# System cards

One card per system, cut by how it fails. `scripts/systems-touched.sh` reads each card's frontmatter to list the cards a change touches, and `/review-loop` wakes a reviewer for each. The settings and the tripwire rule are in `AGENTS.md`.

| Card | Setting | Fails as |
|---|---|---|
| [`harness-files`](harness-files.md) | teach-me | a rule, hook or build script changes what every agent, or the machine, does |
| [`peer-identity-and-routing`](peer-identity-and-routing.md) | teach-me | one peer speaks as another, or a message reaches the wrong peer |
| [`privacy-and-logging`](privacy-and-logging.md) | teach-me | client data reaches the server's logs or disk, or a third party |
| [`peer-grouping`](peer-grouping.md) | teach-me | strangers see each other, or neighbours can't |
| [`wire-format`](wire-format.md) | teach-me | the server and a client stop understanding each other |
| [`state-and-timing`](state-and-timing.md) | teach-me | two connections at once leave the peer map or an inbox in a bad state |
| [`resource-limits`](resource-limits.md) | keep-me-posted | one client uses up memory, connections or time meant for everyone |
| [`frontend`](frontend.md) | trust | the page shows the wrong peers or runs a peer's text as code |

Planned: `content-integrity` (hashing, content IDs, Merkle verification, D10), once `crabshare-core` exists.
