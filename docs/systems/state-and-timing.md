---
name: state-and-timing
title: State and timing
involvement: teach-me
owns:
  - 'src/main.rs'
  - 'src/peer.rs'
watch:
  - '\.lock\(\)|Mutex::new\(|RwLock'
  - 'mpsc::channel|\.send\(.*\)\.await'
  - 'tokio::spawn\(|tokio::time::timeout\('
---

# State and timing

**Involvement:** teach-me. Many connections share one peer map behind one lock, and each peer has a bounded inbox (D6, D7). Tier 3 item 6.
**The question this card asks of every change:** what can another connection do while this one is in the middle of it?

## Invariants (always true)
- The peer map lock is taken in an explicit `{ }` block, and nothing is `.await`ed while it's held.
- Messages to other peers are sent after the lock is released, with `try_send`. A full inbox drops the message and logs it at warn (D7).
- A socket write has a timeout. After a timeout the socket is never written to again.
- On disconnect the inbox is closed first, then the peer is removed, then the others are told.

## Tripwires (changes that pull you in, even from a trusted area)
- A new lock, a new channel, a new spawned task or a new timeout.
- A `send(...).await` to another peer's inbox.

## How it breaks
- A new peer could receive `peer_joined` messages before its `welcome`. Fixed by queuing `welcome` inside the lock, before the peer is inserted (`f4d23e0`).
- rustfmt silently skips a whole `select!` block when one line can't fit in 100 columns, so formatting drift there isn't caught by `cargo fmt --check`.

## Checks
- **Programs:** unit tests for `remove_peer` and `send_to_each` in `src/peer.rs`, including a full inbox and a closed one.
- **Reviewer:** for each `.await` in the changed code, is a lock held? For each send, what if the inbox is full or closed? What does the order of steps guarantee to other peers?

## Briefing
What happens under the lock and what after it, the order of steps on connect and disconnect, and 2 or 3 interleavings of two connections that could go wrong.
