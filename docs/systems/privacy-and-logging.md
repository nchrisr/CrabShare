---
name: privacy-and-logging
title: Privacy and logging
involvement: teach-me
owns:
  - 'src/device.rs'
  - 'src/messages.rs'
watch:
  - 'tracing::\w+!\(.*(",.*\b(sdp|candidate|addr|user_agent|query|payload|text|file_name)\b|\{(sdp|candidate|addr|user_agent|query|payload|text|file_name)[:}])'
  - 'std::fs::|File::create\(|OpenOptions::new\('
  - 'iceServers|"stun:|"turn:'
---

# Privacy and logging

**Involvement:** teach-me. The server is trusted only because it never sees content and logs very little (D1, D9). Tier 3 item 1.
**The question this card asks of every change:** what does the server, or a third party, now learn about a user that it didn't before?

## Invariants (always true)
- The server never stores transferred content, and writes nothing to disk.
- Logs carry only connection events, peer IDs, subnet keys, message types, counts and errors (D9). Never SDP or ICE payloads, file names or sizes, text, or anything from a DataChannel.
- File hashes and Merkle roots never go through signaling (D10).
- `/ws` refusal logs carry a fixed reason label and a count only, never header values or the client address (D15).
- The page contacts no host but the CrabShare server unless a decision says so. A STUN or TURN server learns each user's IP address.

## Tripwires (changes that pull you in, even from a trusted area)
- A log line that formats a client value: an address, a header, a query, a payload.
- Any file write on the server.
- An ICE server in the page's WebRTC config.

## How it breaks
- No incidents yet.

## Checks
- **Programs:** the `watch` patterns catch a client value passed as an argument or inlined as `{name}`, on single-line log calls only.
- **Reviewer:** read every new or changed `tracing::` call, including multi-line ones: which arguments come from the client? Does any new field reach a log through `Debug`?

## Briefing
What data the change reads from the client, where each piece goes (memory, log, another peer, a third party), and how long it stays.
