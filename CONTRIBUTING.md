# Contributing to CrabShare

Thanks for helping. A few things to know before you open a pull request.

## Branches

- **Open pull requests against `develop`.** It's the default branch, and all work lands there first.
- **`main` is for releases only.** The maintainer moves `develop` to `main` with a release pull request. A pull request opened against `main` is retargeted to `develop`.

## Before you open a pull request

Run the same checks CI runs:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/doc-refs-check.py
```

The Rust version is pinned in `rust-toolchain.toml`; `rustup` installs it the first time you run `cargo`. `--locked` means `Cargo.lock` must not change. If your change adds or upgrades a crate, say why in the pull request.

## What to expect

- CI runs on every pull request. For a first-time contributor, the maintainer approves the run after reading the change.
- Every pull request needs the maintainer's approval. A new push after approval needs approving again.
- Pull requests are squash-merged into one commit.
- Changes to CI, scripts, tests or the Rust toolchain get a closer read, because they change what the checks themselves do.

## Ground rules

- `AGENTS.md` has the project's coding conventions and hard constraints. The main one: the server relays connection setup only and never sees or logs what people share.
- Larger changes start with a short design note in `docs/specs/`. Open an issue first if you're unsure.
