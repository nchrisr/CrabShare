# CrabShare

A file and text sharing tool for devices on the same network, built with a Rust core.

Privacy is the point. Files and text travel directly between browsers, encrypted in transit. The server only introduces devices to each other: it never sees, stores, or logs what you share.

**Status:** early development. Devices on the same network already find each other; sending text and files between them is being built. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

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
cargo fmt --check                                     # formatting
cargo clippy --locked --all-targets -- -D warnings    # lints
cargo test --locked                                   # tests
```

CI runs these on every pull request. The Rust version is pinned in `rust-toolchain.toml`, and `rustup` installs it on first use.

## Contributing

Pull requests go to `develop`. See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in CrabShare, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.

## Project docs

- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestones and what's next
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — design decisions and their rationale
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to send a change
- [`AGENTS.md`](AGENTS.md) — orientation and coding conventions, for contributors and AI coding agents
