# Device types and names in the peer list

**Tier:** 3. It changes the wire format, makes the server parse and relay client-supplied data for the first time, stores a user-chosen name on the server while a peer is connected, and adds a presence message under the full-inbox rule (AGENTS.md tier 3, items 1, 2, 4 and 6).

**Why:** The peer list shows generated names such as "Calm Quokka". With several devices on one network, people can't tell which entry is their own phone and which is someone else's laptop. A web page can't read the device's real name (browsers don't expose it, so sites can't use it to track people), so the list needs two things it doesn't have: the kind of device, and a name the person chooses.

## Decisions

### 1. Device type: declared by the client if it can, else read from the `User-Agent` header

- **Options:** (a) the browser reads `navigator.userAgent` and sends its type in a message; (b) the server reads the `User-Agent` header on the WebSocket upgrade request; (c) the client may declare its type as a `device` query parameter on the upgrade URL (`/ws?device=iphone`), and the server falls back to (b) when it is missing or not a known value.
- **Chosen:** (c). Native apps (a possible later client) know their device type exactly and don't send a browser-style user agent, so (b) alone would show them as Unknown. A query parameter can be set by both a browser and a native app, needs no new message, and is read before the upgrade, so the type is known before `welcome` is sent. Only values from the fixed list (decision 2) are accepted; anything else falls back to the user agent. A malformed query string or a repeated `device` key also falls back: the upgrade is never refused over this parameter, so the server reads the raw query and takes the first `device` value rather than using an extractor that rejects with a 400. The web client sends no `device` parameter for now. Both parsers are pure Rust functions with unit tests.
- **Still uncertain:** iPads running Safari send a Mac user agent by default, so they show as Mac. Fixing that needs a browser-side check (touch support), which could later use the same query parameter, and is left out.

### 2. A fixed set of device types

- **Options:** send the raw user agent to other peers, or reduce it to a short list.
- **Chosen:** a `DeviceKind` enum: `iphone`, `ipad`, `android`, `mac`, `windows`, `linux`, `unknown`. The raw string is never stored, sent or logged: it carries version details that help fingerprint a device. The browser maps each kind to a label and an icon from a fixed table, with a fallback for values it doesn't know.
- **Still uncertain:** nothing.

### 3. The default name stays in the browser

- **Options:** (a) the server generates a default name; (b) the server stores no name until the person sets one, and each browser shows the generated name (from the peer ID, as today) when there is none.
- **Chosen:** (b). Every browser already derives the same generated name from the same ID, so the server doesn't need to send one. The server only ever holds a name the client sent with `set_name` (decision 4). It doesn't know or care where the name came from: typed in the web page, or, in a later native app, read from the device's own name.
- **Native clients:** a device's own name often contains a person's real name ("Chris's iPhone"), and `set_name` shows it to everyone on the network. A native app should pre-fill the name and let the person confirm before sending it, never send it silently. Recent iOS versions return only a generic name such as "iPhone" without a special entitlement, so the generated-name fallback still matters there.
- **Still uncertain:** nothing.

### 4. Typed names: a new client message, checked by the server

- **Options:** keep names entirely in the browser (each device could only name itself on its own screen), or send them through the server to the other peers.
- **Chosen:** send them. New message from browser to server: `set_name { name }`. It has no ID field: the server always applies it to the sender. The server cleans the name (decision 5), ignores it if unchanged, stores it on the sender's map entry, and broadcasts `peer_updated { peer_id, name }` to the whole subnet **including the sender**, so the sender's own screen shows exactly the cleaned name everyone else sees.
- **Still uncertain:** nothing.

### 5. Cleaning names

- **Options:** remove only control characters (`char::is_control`), or also the invisible and direction-changing characters that let a name look blank or rearrange nearby text.
- **Chosen:** the second, in this order:
  1. remove control characters, format characters that change text direction or are invisible (U+200B to U+200F, U+202A to U+202E, U+2060 to U+2069, U+FEFF), and line and paragraph separators (U+2028, U+2029);
  2. trim whitespace;
  3. cap at 40 characters (Rust `char`s, so a multi-byte character is never split);
  4. trim again;
  5. if nothing visible is left, treat it as "no name".
- **Still uncertain:** stacked accent marks can still draw outside their row; the page clips each name to its row instead (decision 9).

### 6. At most one rename per second per connection

- **Options:** (a) no limit beyond "ignore if unchanged"; (b) at most one accepted rename per second, others ignored; (c) the same, but keep the latest and send it when the second is up.
- **Chosen:** (b). Without a limit, a tab alternating between two names makes the server send a `peer_updated` to every peer for every frame, filling their inboxes so real `peer_joined`, `peer_left` and, later, connection setup messages get dropped. (c) needs a timer in the connection loop; (b) is enough because the page only sends `set_name` when someone submits a new name. An ignored rename is logged at debug by peer ID.
- **Still uncertain:** a general limit on incoming frames per connection is still needed and belongs with the routing change.

### 7. Additive wire changes, so open tabs keep working

- **Options:** (a) change `welcome.peers` and `peer_joined` to carry objects; (b) only add fields.
- **Chosen:** (b). A tab that is already open keeps its old `app.js` across reconnects, and would break on (a). With (b) it ignores the fields it doesn't know.
  - New type `PeerInfo { peer_id, device, name }`, where `name` is `null` when unset.
  - `welcome` gains `device` (this device's own kind) and `peer_details: [PeerInfo]`; `peers` stays a list of IDs.
  - `peer_joined` gains `device` and `name`.
  - New `peer_updated { peer_id, name }`.
  - New optional `device` query parameter on `/ws` (decision 1). Existing clients don't send it.
- **Still uncertain:** nothing.

### 8. Parsing client messages safely

- **Options:** parse each text frame into `SignalingMessage`, or into a separate client-to-server enum.
- **Chosen:** a separate `ClientMessage` enum with the same `#[serde(tag = "type", rename_all = "snake_case")]` scheme (D8). It holds only what a browser may send: `set_name` now, and later the relay messages. A browser can then never send a server-only message such as `welcome`. A frame that fails is skipped and logged at debug with a fixed category only: "not JSON or unknown type", or "binary frame". The serde error is never formatted into a log, because its text includes the client's input. The connection keeps running.
- **Still uncertain:** the routing change will add `offer`, `answer` and `ice_candidate` to this enum.

### 9. Display rules in the page

- **Chosen:**
  - Names are inserted only as text (`textContent`), never through `innerHTML`, `insertAdjacentHTML` or HTML template strings, so a name can't run script on other devices.
  - Each name sits in its own isolated text run (`unicode-bidi: isolate`) and is clipped to one line, so it can't rearrange or draw over the rest of the list.
  - A typed name is always shown with the generated name or short ID beside it. Names and device types are claims a device makes about itself, not verified identity, and anyone can copy someone else's name. The short ID lets people tell two "Alice's MacBook"s apart. Verified pairing is a later change.
  - The rename control says the name is visible to everyone on this network.
- **Still uncertain:** nothing.

### 10. A size limit on incoming messages

- **Options:** keep the library defaults (tens of MB per message), or set a limit sized for signaling.
- **Chosen:** 16 KB per message and per frame, set on the upgrade. An SDP offer is a few KB. A message over the limit ends that connection; this is deliberate, since no real client sends one. It is logged at info as "message too large", with the peer ID.
- **Still uncertain:** a full (non-trickle) SDP offer can be larger than a trickle one. The routing change must check real offers against this limit.

### 11. Names and user agents are never logged

- **Chosen:** log lines may say a peer renamed itself, by ID only. The raw user agent is never logged; the `DeviceKind` may be. The same goes for the raw `device` query value and the query string as a whole: only the resulting `DeviceKind`, and whether it was declared or taken from the user agent, may be logged. `ClientMessage` gets a `type_name()` like `SignalingMessage`, and neither is logged with `{:?}`. D9 is updated to say so.
- **Still uncertain:** nothing.

## Changes

`src/messages.rs`
- New `enum DeviceKind { Iphone, Ipad, Android, Mac, Windows, Linux, Unknown }`, serialized snake_case.
- New `struct PeerInfo { peer_id: Uuid, device: DeviceKind, name: Option<String> }`.
- `Welcome` gains `device: DeviceKind` and `peer_details: Vec<PeerInfo>`.
- `PeerJoined` gains `device: DeviceKind` and `name: Option<String>`.
- New variant `PeerUpdated { peer_id: Uuid, name: Option<String> }`; `type_name()` gains `"peer_updated"`.
- New `enum ClientMessage { SetName { name: String } }`, deserialize only, with its own `type_name()`.

`src/peer.rs`
- `Peer` gains `device: DeviceKind`, `name: Option<String>` and `last_rename: Option<Instant>`, and a method `fn info(&self) -> PeerInfo`.

`src/device.rs` (new)
- New `fn connecting_device_kind(query: Option<&str>, user_agent: Option<&str>) -> DeviceKind`, combining the two below with the query value taken by `device_query_value` (first `device=` wins).
- New `fn device_kind(user_agent: &str) -> DeviceKind`, checked in this order: iPad, iPhone, Android, Mac, Windows, Linux, else Unknown.
- New `fn declared_device_kind(value: &str) -> Option<DeviceKind>`: an exact match on the snake_case names from decision 2, else `None`. `unknown` counts as not declared.

`src/main.rs`
- `ws_handler` reads an optional `device` query parameter. If `declared_device_kind` accepts it, that is the device type; otherwise `device_kind` on the user agent.
- New `fn clean_name(raw: &str) -> Option<String>`, following decision 5.
- `ws_handler` reads the `User-Agent` header (missing or not valid text means Unknown) and sets the 16 KB message and frame limits before upgrading.
- The join path fills `welcome`'s new fields and `peer_joined`'s.
- The `select!` loop's browser branch parses text frames as `ClientMessage`. On `SetName`: clean the name; inside the lock, look up the sender's entry (skip if missing), skip if the name is unchanged or the last rename was under a second ago, otherwise update the name and time and collect every peer on the subnet; after the lock, `send_to_each` a `PeerUpdated`.
- A receive error that means "message too large" is logged before the loop ends.

`static/app.js`, `static/index.html`, `static/style.css`
- Keep peers as a map from ID to `{ device, name }`, filled from `peer_details` and `peer_joined`.
- Show a device icon and label from a fixed table, the typed name if any, and the generated name or short ID beside it.
- This device's own name has a rename control and a note that it's visible to everyone on the network. The name is saved in `localStorage` and sent with `set_name` after each `welcome` and on rename. The display follows `peer_updated` for the device's own ID.
- `peer_updated` for an unknown ID is ignored.
- Names only ever set through `textContent`; isolated and clipped as in decision 9.

`docs/DECISIONS.md`
- D7: `peer_updated` follows the full-inbox rule, and renames are limited to one per second per connection.
- D8: current message list, the additive-change rule, the `ClientMessage` split, and the optional `device` query parameter on `/ws` (values are the `DeviceKind` names, add-only; unknown or malformed values fall back to the user agent).
- D9: names, raw user agents, the raw `device` value and the query string are never logged.

## Testing

Unit tests in `src/device.rs` and `src/main.rs`:
- `device_kind` against real user-agent strings: iPhone Safari, iPad (both the iPad and the desktop-mode Mac string), Android Chrome, Mac Safari and Chrome, Windows Edge, Linux Firefox, an empty string.
- `declared_device_kind`: accepts each known name; rejects `unknown`, an empty string, a different case (`iPhone`), and an unknown value.
- `clean_name`: trims; removes control characters, newlines, zero-width characters and direction overrides; a name made only of zero-width spaces, or of control characters and spaces, becomes `None`; caps at 40 characters without splitting a multi-byte character; no trailing space after the cap.

Unit tests in `src/messages.rs`:
- `type_name` still matches the serialized tag for every variant, including `peer_updated`; the same for `ClientMessage`.
- `ClientMessage` parses `{"type":"set_name","name":"x"}`, and rejects `{"type":"welcome",...}` and unknown types.

By hand:
- Two real devices on the same Wi-Fi: each shows the other's device type; renaming one updates the other's list within a second; reloading keeps the name.
- An already-open tab running the old `app.js` keeps showing its list after the server restarts with this change.
- `websocat`: send a non-JSON line and a `{"type":"welcome"}` line; the connection stays open and the log has only the category.
- `websocat`: send several different `set_name`s in under a second; only the first is broadcast.
- `websocat ws://127.0.0.1:3000/ws?device=android`: the other peers see Android; with `?device=toaster` the type falls back to the user agent, and `?device=a&device=b` still connects, using the user agent.
- `websocat`: send a 20 KB line; the connection closes with a "message too large" log, and the server keeps running.
- A name containing `<img src=x onerror=alert(1)>` shows as plain text on the other device.

Not checked here: browsers other than Safari and Chrome, and iPads in desktop mode beyond the known limit.

**Known limits:** names are visible to everyone grouped with the device. On a self-hosted LAN that is the local network. Behind a reverse proxy, before Milestone 7 (M7, public-hosting readiness), every client shares one group, so a public instance must not run until M7 lands.

## Owner steps

1. Review and approve this spec.
2. Implement it, or hand pieces over.
3. `cargo test`, `cargo clippy`, `cargo fmt --check`.
4. Run the hand checks on two real devices.
