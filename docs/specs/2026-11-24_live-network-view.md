# Live network view

**Tier:** 2. A new feature that lives only in `static/`. It sends nothing new to the server, adds no messages, and changes no server code, so none of the tier 3 items apply.
**Systems:** frontend (trust). The card's tripwire (writing HTML from a string) is avoided by design (decision 2), so it stays at trust.
**Why:** The page is a plain list of names. It doesn't show what CrabShare actually does, so people have to take the privacy promise on trust: that the server only introduces devices, and content goes directly from one device to another. A live picture of the network, with the server's role drawn separately from the direct device-to-device path, makes that visible, and gives a place to show connection details as signaling (Milestone 2, M2) and the DataChannel (Milestone 4, M4) land.

## Decisions

### 1. Layout: peers on a ring, the server outside it

- **Options:** (a) the server at the centre of the ring, peers around it; (b) the server as a node outside the ring, above it.
- **Chosen:** (b). With the server at the centre, a direct transfer between two opposite peers would cross the centre and look as if it went through the server. With the server outside, the ring reads as "your network": signaling paths leave the ring to reach the server, and content paths are chords that stay inside the ring. This device sits at the bottom of the ring and is marked "You"; the others are spread evenly, sorted by ID so the order doesn't jump on every join.
- **Still uncertain:** past about 12 peers the nodes get crowded. The ring shrinks node labels to icons past that point; the full list below the ring (decision 7) always has everyone.

### 2. Drawing: HTML nodes over an SVG layer, built with DOM calls only

- **Options:** (a) one SVG drawn from a template string with `innerHTML`; (b) an SVG built with `createElementNS`; (c) nodes as HTML buttons positioned over an SVG layer that holds only the lines and moving packets.
- **Chosen:** (c). Nodes as HTML get CSS text handling for names (isolation, ellipsis, decision 9 of the device names spec), are focusable and tappable, and are easy to read for screen readers. The SVG layer holds paths and packet dots, created with `createElementNS`. Icons are `<symbol>`s written once in `index.html` and referenced with `<use href="#icon-phone">`, so no markup is ever built from a string. Every name and label goes in with `textContent`.
- **Still uncertain:** nothing.

### 3. Device icons from a fixed table

- **Options:** guess the type in the browser from `navigator.userAgent`, or use the `device` field the server will send (device names spec, decision 7).
- **Chosen:** the server's field when present, mapped through a fixed table: `iphone` and `android` to a phone, `ipad` to a tablet, `mac`, `windows` and `linux` to a computer, anything else (including a missing field or a value added later) to an "unknown device" icon. The server is the single source, so every tab shows the same icon for the same peer. Until the server sends `device`, every peer shows the unknown icon. This device's own icon may use a local guess from `navigator.userAgent` until `welcome.device` arrives; the guess never leaves the page.
- **Still uncertain:** iPads in desktop mode show as a computer (known limit from the device names spec).

### 4. One internal event shape for everything that moves

- **Options:** animate directly inside each message handler, or route every movement through one function.
- **Chosen:** one function, `showTraffic(event)`, with two kinds:
  - `{ kind: "signal", type, from, to }`: drawn as a packet from `from` to the server, then from the server to `to`. `from` or `to` may be `"server"` (for example `welcome` goes server to you).
  - `{ kind: "data", from, to, label, progress }`: drawn as a stream of packets along the chord between the two peers, never touching the server. `progress` (0 to 1, or `null` when unknown) fills a bar on the chord.
  Today `welcome`, `peer_joined` and `peer_left` call it with `kind: "signal"`. When M2 adds `offer`, `answer` and `ice_candidate`, their handlers call it the same way. When M4 and M5 add the DataChannel, the channel code calls it with `kind: "data"`. No drawing code changes for either.
- **Still uncertain:** whether the page should also show signaling it sends itself (offers it makes). Yes when M2 lands, by calling the same function on send.

### 5. What a join and a leave look like

- **Chosen:** `peer_joined`: the new node fades in, a packet goes from it to the server, then the server sends one packet to each other peer. `welcome`: the ring fills in, then one packet goes from the server to you (the packet needs your place on the ring). `peer_left`: one packet from the server to each remaining peer, then the node fades out. Each tab can only see its own messages, so the fan-out to other peers is drawn from what the server is known to do (D7: presence goes to the whole subnet), and the panel says so.
- **Still uncertain:** nothing.

### 6. No demo mode

- **Options:** (a) animations only for real events; (b) a demo switch that plays simulated offer, answer, ICE and a transfer.
- **Chosen:** (a). Simulated traffic on a page whose purpose is to show what really happens risks being mistaken for the real thing. The DataChannel animation waits for real transfers (M4, M5); until then only joins and leaves move.
- **Still uncertain:** nothing.

### 7. A text list and an activity feed alongside the picture

- **Chosen:** below the ring, the peer list stays as text (icon, name, short ID), matching `welcome`, `peer_joined` and `peer_left` as the frontend card requires. A short activity feed shows the last 20 events by message type and peer name only ("peer_joined: Calm Quokka"), never payloads, so it follows the same line as server logs (D9) once SDP and ICE exist.
- **Still uncertain:** nothing.

### 8. A "How this connection works" panel

- **Chosen:** a panel with two rows, each with a live state:
  - **Signaling:** WebSocket to this server, the scheme shown honestly: `ws://` is "not encrypted (local network)", `wss://` is "encrypted (TLS)". State: connecting, connected, reconnecting. One line: "The server only introduces devices. It never sees what you share."
  - **Content:** WebRTC DataChannel, "direct between devices, encrypted with DTLS". State: "not built yet" until M4. Tapping a peer selects it, and the row then shows that pair's channel state and, from `RTCPeerConnection.getStats()`, the ICE candidate type in plain words: `host` "same network, direct", `srflx` "direct, through your router's public address", `relay` "through a relay server". Until M4 the per-peer view shows "No direct connection yet".
  - A small note shows whether the page runs in a secure context (`window.isSecureContext`), because browsers only allow WebRTC there. On `http://192.168.x.x` this answers the open "secure context on LAN" question visibly.
- **Still uncertain:** nothing.

### 9. Phone width and reduced motion

- **Chosen:** the ring is a square that fills the column width (max 480 px, as now) and scales with it; node labels shrink and then hide below 360 px, leaving icons. The panel and list stack below. With `prefers-reduced-motion`, packets don't travel: the path briefly highlights instead.
- **Still uncertain:** nothing.

### 10. Device names part 2: render when present

- **Chosen:** peers are kept as a map from ID to `{ device, name }`. Fields come from `welcome.peer_details`, `peer_joined.device` and `.name`, and `peer_updated` when the server sends them, and are `null` until then. A typed name shows with the generated name or short ID beside it (device names spec, decision 9). The rename control from that spec is built here, in a later change, once the server side lands.
- **Still uncertain:** nothing.

## How it could go wrong

The only card is frontend (trust), so this is for the demo rather than a briefing.
- A peer name or label ends up as HTML: caught by building only with DOM calls and `textContent`, the card's watch pattern, and the reviewer.
- The picture and the list disagree with the server (a ghost node after a leave, a missing node after reconnect): caught by the two-tab join, leave and server-restart check below.

## Changes

`static/index.html`
- Icon `<symbol>`s: server, phone, tablet, computer, unknown.
- Sections: header with connection status; the ring stage (an SVG layer and a node layer); the "How this connection works" panel; the peer list; the activity feed.

`static/app.js`
- State: `myId`, `myDevice`, `peers: Map<id, { device, name }>`, `selectedPeer`.
- `handle(message)`: as now, plus filling `device` and `name` when present, and calling `showTraffic` for each message.
- `layoutRing(ids) -> Map<id, { x, y }>`: positions as fractions of the stage, you at the bottom, others by sorted ID. Pure function.
- `iconFor(device) -> "phone" | "tablet" | "computer" | "unknown"`: the fixed table. Pure function.
- `render()`: rebuilds nodes, list and panel from state with DOM calls only.
- `showTraffic(event)`: the single entry point for animations (decision 4); caps moving packets, then highlights paths instead, then drops legs past a second cap, so a burst doesn't flood the stage.
- `describeCandidateType(type) -> string`: plain-words table for decision 8, ready for M4.

`static/style.css`
- Ring stage, node, server and selected styles; packet styles; panel, list and feed; narrow-width and reduced-motion rules. Light and dark, using the existing colour variables.

## Testing

- Two tabs on `http://localhost:3000`: the second tab appears on the first tab's ring with a join animation through the server; closing it animates a leave and removes it from both the ring and the list.
- Restart the server: both tabs show reconnecting, then rebuild the same picture.
- Phone width (375 px) in the browser pane, and an actual phone on the LAN: ring, panel and list fit with no sideways scroll.
- Reduced motion on: no travelling packets.
- No new requests to the server beyond the existing `/ws` and static files (network tab).
- Not checked here: the DataChannel animation and the ICE rows with real data, which wait for M4 and M5.

## Owner steps

1. Review and approve this spec. Approved 2026-11-24, with the demo mode dropped.
2. After the build: `cargo run` from the repo root, open two tabs on `http://localhost:3000`, and try a join and a leave.
