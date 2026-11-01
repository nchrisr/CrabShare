# CrabShare

A file and text sharing tool for devices on the same network, built with a Rust core.

Privacy is the point. Files and text travel directly between browsers, encrypted in transit. The server only introduces devices to each other: it never sees, stores, or logs what you share.

## How it works

Devices connect to a signaling server. Peers discover each other automatically by subnet and establish direct WebRTC connections for file and text transfer — no accounts, no cloud, no relay.

```
                    SIGNALING SERVER
   ┌─────────────────────────────────────────────┐
   │ peers, grouped by subnet                    │
   │                                             │
   │   192.168.1.x ──► { A, B, C }               │
   │   10.0.0.x    ──► { D }                     │
   └─────────────────────────────────────────────┘
        ▲ │         ▲ │         ▲ │        ▲ │
        │ ▼         │ ▼         │ ▼        │ ▼   one WebSocket per device
   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ A       │ │ B       │ │ C       │ │ D       │
   │ sees:   │ │ sees:   │ │ sees:   │ │ sees:   │
   │  B, C   │ │  A, C   │ │  A, B   │ │ nobody  │
   └─────────┘ └─────────┘ └─────────┘ └─────────┘
   └───── same network (192.168.1.x) ────┘  other network
```

- **The server** keeps the list of connected devices, grouped by network. Devices only ever see others on their own network.
- **Each device** gets a `welcome` with the devices already there when it connects, then a `peer_joined` or `peer_left` whenever that changes.
- **To share**, two devices swap WebRTC connection details through the server, then open a direct, encrypted connection. Files and text go over that connection and never touch the server.

## Stack

- **Backend:** Rust, [axum](https://github.com/tokio-rs/axum), tokio
- **Frontend:** HTML, CSS, JavaScript
- **Transfer:** WebRTC DataChannel (peer-to-peer)

## Running locally

```bash
cargo run
```

Then open `http://localhost:3000` in your browser.

## Development

```bash
cargo fmt       # format code
cargo clippy    # lint
cargo test      # run tests
```

## Project docs

- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestones and what's next
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — design decisions and their rationale
- [`AGENTS.md`](AGENTS.md) — orientation and coding conventions, for contributors and AI coding agents
