# Broadcast `peer_left` on disconnect

**Tier:** 3. It reads and changes the shared peer map, decides which peers get told about another peer, and changes how full inboxes are handled (AGENTS.md tier 3, items 2 and 6).

**Why:** When a device closes its tab or loses its connection, the server removes it from the peer map, but nobody else on the subnet is told. Every other device keeps showing it as available, and any offer sent to it would go nowhere. Milestone 2 (M2, signaling) needs join and leave both broadcast, and the browser peer list in Milestone 3 (M3) depends on it. The `PeerLeft { peer_id }` variant already exists in `src/messages.rs` but is never sent.

## Decisions

### 1. Remove the peer and pick the recipients in one lock

- **Options:** (a) remove the peer in one lock block, then take the lock again to list who is left; (b) do both in the same block.
- **Chosen:** (b). Between two lock blocks, a new peer can join and an old one can leave, so the list would describe a different moment than the removal. In one block, every peer is in exactly one of two states: it joined before the removal (it got a `peer_joined` or `welcome` that included the leaver, and now gets `peer_left`), or after it (it never heard of the leaver and hears nothing). This matches how the join path already works.
- **Still uncertain:** nothing.

### 2. Move the removal into a function that doesn't touch the network

- **Options:** (a) keep the logic inline in the connection task, as the join path is; (b) a plain function that takes the map by `&mut`, removes the peer, drops the subnet entry if it is now empty, and returns the peers still on that subnet.
- **Chosen:** (b). It holds no lock and does no `.await`, so it can be unit-tested with a hand-built map, which nothing outside `subnet_key` has today. The connection task calls it inside its lock block, passing the locked map.
- **Still uncertain:** whether the join path should get the same treatment. Left for a later change.

### 3. Never wait on a full inbox; skip it

- **Options:** (a) `send().await`, as `peer_joined` does today; (b) `try_send`, and on a full inbox log it and drop the message.
- **Chosen:** (b), for both `peer_joined` and `peer_left`. This settles Decision 7 (D7 in `docs/DECISIONS.md`, the per-peer inbox) for presence messages. Waiting is worse than it looks:
  - **Two tasks can wait on each other forever.** B joins and blocks sending `peer_joined` to A, whose inbox is full. B isn't reading its own inbox yet, because its `select!` loop hasn't started. A then disconnects and blocks sending `peer_left` to B, whose inbox is also full. Neither ever finishes, and every later join or leave on that subnet that sends to either of them hangs too.
  - **One stuck socket stalls every newcomer.** If one peer's socket hangs, its inbox fills. Every new tab on that subnet then waits on it before its own `welcome` is written, so it sits with an empty list.
- **Still uncertain:** a healthy peer that overflows during a burst (for example everyone reconnecting after a server restart, with more than 32 peers on one subnet) misses an update and its list goes stale. A later fix could send it a fresh snapshot instead of queued events. Relay messages (`offer`, `answer`, `ice_candidate`) get their own decision in the routing spec.

### 4. A time limit on socket writes, as a backstop

- **Options:**
  - (a) do nothing more: a stuck peer stays until TCP gives up, which can take many minutes, or forever if the client keeps the connection open but stops reading;
  - (b) a kick signal (`Arc<Notify>`) that other tasks pull when they find a full inbox;
  - (c) wrap each socket write in `tokio::time::timeout`, so a write that takes too long ends the peer's own task.
- **Chosen:** (c), with a 10 second limit. With (b), a full inbox means the task is stuck inside a socket write, so it never returns to its `select!` loop to see the kick, and a burst could kick healthy peers. With (c), a task whose write blocks frees itself: the write times out, the loop ends, and the normal cleanup tells everyone it left. A healthy peer is never disconnected, because its writes finish quickly. tokio's `time` feature is already on (`full`).
- **What it does not catch:** a write only blocks once the operating system's send buffer is full, which takes a lot of data. A peer that simply vanishes (leaves Wi-Fi, sleeps) keeps accepting writes instantly, so the limit never fires and the peer stays listed. Only a peer that stays connected but stops reading is caught. Vanished peers need a server heartbeat (ping every N seconds, disconnect if nothing comes back), which is its own change.
- **After a timeout:** never write to the socket again, not even a close frame. The timed-out write may have left half a frame on the wire. Leave the loop and let the socket drop.
- **Still uncertain:** whether 10 seconds is right. Reconnecting is the browser's job, part of the M3 frontend, and should back off with some randomness so many tabs don't reconnect at the same instant.

### 5. Close the leaver's inbox before broadcasting

- **Options:** keep the inbox `Receiver` alive until the task ends, or drop it as soon as the `select!` loop exits.
- **Chosen:** drop it first. Other tasks may still hold a copy of the leaver's `Sender` from an earlier snapshot. Once the `Receiver` is gone, their sends fail at once instead of filling a queue nobody reads.
- **Still uncertain:** nothing.

### 6. One helper for sending a message to many peers

- **Options:** (a) copy the join path's send loop; (b) one helper used by both broadcasts.
- **Chosen:** (b). Two copies of the same loop drift apart, and both follow the rule in decision 3. Recipients are `Peer` values: `Peer` gains `Clone`, which copies the ID and clones the `Sender` handle, both cheap. Sending one message to many peers needs a copy per recipient, so `SignalingMessage` gains `Clone` too.
- **Still uncertain:** nothing.

### 7. Log message types with a method, never with `Debug`

- **Options:** log the message with `{:?}`, or add a method that returns only the wire tag.
- **Chosen:** a method. `{:?}` on `Offer`, `Answer` or `IceCandidate` prints the SDP or candidate, which Decision 9 (D9, privacy and logging) forbids. The helper is shared, so it will see those variants once routing lands.
- **Still uncertain:** nothing.

### 8. Remove `Peer.subnet`

- **Options:** keep it for later use, or remove it.
- **Chosen:** remove. The connection task already holds its subnet, and the map key stores it too, so the field is a third copy that nothing reads. It is clippy's only warning. Message routing (the next M2 change) uses the sender's own subnet, so it won't need it either.
- **Still uncertain:** nothing.

## Changes

`src/messages.rs`
- `SignalingMessage` derives `Clone` as well.
- New method `fn type_name(&self) -> &'static str`, returning the same tag serde writes, for example `"peer_left"`. It matches every variant by name with no `_` arm, so a new variant can't compile until it has a name. All logging of a message uses this.

`src/peer.rs`
- `Peer` derives `Clone`.
- `Peer` loses its `subnet` field.

`src/main.rs`
- New function:
  `fn remove_peer(peers: &mut HashMap<String, HashMap<Uuid, Peer>>, subnet: &str, peer_id: Uuid) -> Vec<Peer>`
  Removes the peer, drops the subnet entry if it is now empty, and returns the peers left on that subnet. Removing a peer that isn't there returns an empty list and changes nothing. Other subnets are never touched.
- New function:
  `fn send_to_each(recipients: Vec<Peer>, message: SignalingMessage) -> usize`
  Offers a copy of `message` to every recipient with `try_send`, never waiting, so it is not `async`. Returns how many sends succeeded.
  - On a full inbox: log at warn that the inbox was full and the message dropped, since that peer's list is now wrong.
  - On a closed inbox: log at debug that the peer already disconnected.
  - Logs give the recipient's ID and `message.type_name()`, never the message itself.
- New constant `SOCKET_WRITE_TIMEOUT: Duration`, 10 seconds.
- The join broadcast collects `Peer` clones instead of `(id, sender)` pairs, and uses `send_to_each`.
- In the `select!` loop's inbox branch, the socket write is wrapped in `tokio::time::timeout(SOCKET_WRITE_TIMEOUT, ...)`. If it times out, log at info that the write timed out (peer ID only) and leave the loop, the same as a failed write.
- After the loop exits, drop `inbox` first. Then the cleanup calls `remove_peer` inside its lock block, and after the block sends `PeerLeft { peer_id }` to the returned peers with `send_to_each`.
- The disconnect log line stays at info and adds the number of peers sent `peer_left` successfully.

`AGENTS.md`
- "Where things live": `src/peer.rs` becomes "Peer: id, outbound channel sender".

`docs/DECISIONS.md`
- D7: presence broadcasts use `try_send` and drop on a full inbox, and socket writes time out after 10 seconds as a backstop for peers that stop reading, with the reasons from decisions 3 and 4. Relay messages are still open.

**Known limits:**
- A peer that vanishes without closing its connection stays listed until a heartbeat exists (see decision 4).
- Because a full inbox drops messages, a peer can get `peer_left` for an ID whose `peer_joined` it never got. The M3 frontend must ignore `peer_left` for an unknown ID.
- `peer_left` is sent only if the connection task reaches its cleanup. If the task panics or is cancelled, the peer stays in the map and nobody is told. Nothing triggers this today; making cleanup run on every exit is a separate change.

## Testing

Unit tests in `src/messages.rs`:
- for one value of each variant, `type_name()` equals the `type` field of its serialized JSON

Unit tests for `remove_peer` in `src/main.rs`, using a hand-built map (creating an `mpsc::channel` needs no runtime):
- removes the peer and returns the others on its subnet, not itself
- drops the subnet entry when the last peer leaves, and returns an empty list
- leaves other subnets untouched
- an unknown peer ID changes nothing and returns an empty list

Unit tests for `send_to_each` (no runtime needed, since it never waits):
- every recipient with room gets the message, and the count matches
- a recipient whose inbox is full (a channel of capacity 1, already holding a message) is skipped, not counted, and still holds only the original message
- a recipient whose `Receiver` was dropped is skipped and not counted

By hand, with three `websocat ws://127.0.0.1:3000/ws` clients (all on subnet `127_0_0`):
- close one with Ctrl-D (clean close) and check the other two each get `{"type":"peer_left","peer_id":"<its id>"}`
- close one with Ctrl-C (abrupt) and check the same
- close the last one and check the server log shows no active peers on `127_0_0`
- a peer that joins and leaves straight away: the others get `peer_joined` before `peer_left`. This holds because one task sends both, in order, as long as neither is dropped for a full inbox.

Not checked here: the write timeout firing (it takes a client that keeps the connection open but stops reading, which `websocat` can't easily fake), a real full inbox end to end, and many simultaneous disconnects.

## Owner steps

1. Implement the changes above.
2. `cargo test`, `cargo clippy` (expect zero warnings), `cargo fmt --check`.
3. Run the `websocat` checks.
4. Update D7 and the AGENTS.md line.
