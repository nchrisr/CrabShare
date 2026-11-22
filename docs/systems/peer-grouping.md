---
name: peer-grouping
title: Peer grouping
involvement: teach-me
owns:
  - 'src/subnet.rs'
watch:
  - '\.ip\(\)|ConnectInfo\(|IpAddr::V6\('
  - 'headers\.get\(.*(forwarded|real.ip)|from_static\("x-(forwarded|real-ip)'
---

# Peer grouping

**Involvement:** teach-me. The grouping decides who can see whom. Tier 3 item 3.
**The question this card asks of every change:** after this, could two strangers land in the same group, or two neighbours in different ones?

## Invariants (always true)
- Peers are grouped by the `/24` of their IPv4 address (D4). IPv6 gets no group (D5).
- The address used is the TCP peer address. No header from the client is trusted for it until trusted-proxy handling exists (M7).

## Tripwires (changes that pull you in, even from a trusted area)
- Reading the client's IP, or reading a proxy header.
- Any change to IPv6 handling.

## How it breaks
- No incidents yet. Behind a reverse proxy every client has the proxy's address, so everyone shares one group (D3, M7).

## Checks
- **Programs:** unit tests for `subnet_key` in `src/subnet.rs`.
- **Reviewer:** which address does the change group by, and can the client influence it? What does a client on a phone network, a VPN or behind a proxy get?

## Briefing
Which address is used, how it becomes a group key, and who ends up seeing whom in 2 or 3 real network setups.
