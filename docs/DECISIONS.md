# Design decisions

Each entry: the context, what was decided, why, and whether it's settled. Add a new entry when a choice is made that a future reader might otherwise second-guess or accidentally undo.

Status legend: **settled** — deliberate, change only with a good reason. **first-pass** — it works, chosen without much deliberation, open to revisiting.

---

## D1. Signaling server + browser-to-browser WebRTC; server never touches content — settled

**Context.** Need to move text and files between devices without accounts or cloud storage. Privacy is a core requirement.

**Decision.** A Rust server does only *signaling*: it tells peers about each other and relays the SDP offer/answer and ICE candidates needed to set up a WebRTC connection. The actual transfer goes over a WebRTC DataChannel directly between browsers.

**Why.** The server never sees the data, so "we don't store your data" is true by construction, not by policy. This is also how Snapdrop works, so the model is proven for this use case.

**Consequences.** No transfer history, no resumable uploads via the server, no server-side scanning. If two peers can't reach each other directly (symmetric NAT), a TURN relay would be needed — deliberately out of scope for now.

---

## D2. Mostly Rust — settled

**Context.** The server is a long-running network service that self-hosters run on their own machines.

**Decision.** Backend in Rust (axum + tokio). Frontend uses web technologies as needed; no constraint on framework choice there.

**Why.** A single static binary is easy for self-hosters to run.

**Extended by D10.** Rust also runs in the browser, compiled to WebAssembly, for file hashing and verification.

---

## D3. Two deployment modes: self-hosted LAN and public instance — settled

**Context.** Anyone should be able to (a) run it on their own machine and reach it from their phone via the machine's LAN IP, or (b) use a public URL that offers the same privacy.

**Decision.** Both are supported targets. The server binds `0.0.0.0` so LAN devices can reach it.

**Consequences.**
- Peer discovery must work when clients connect directly (LAN, `ConnectInfo` gives the real client IP) **and** when they connect through a reverse proxy (public hosting, `ConnectInfo` gives the proxy's IP for everyone). The second case needs `X-Forwarded-For` / `Forwarded` header handling, gated so it's only trusted when actually behind a proxy. **Not yet implemented — planned for M7, see ROADMAP.md.**
- TLS: browsers require a secure context for some WebRTC features. On a public host TLS comes from the proxy; on a LAN `http://192.168.x.x` counts as insecure — this needs investigating before the WebRTC work. **Open question.**

---

## D4. Peers grouped by `/24` subnet of the client IP — first-pass

**Context.** Devices should only see other devices "near" them, not every user of a public instance.

**Decision.** `subnet_key()` takes the first three octets of the client's IPv4 address (`192.168.1.5` → `192_168_1`). Peers are stored per subnet key and will only be announced to each other within the same key.

**Why (first-pass).** Simple, no user input needed, and matches Snapdrop's model. On a public instance, devices behind the same home router share one public IP and therefore one key, which is the desired grouping.

**Alternatives worth revisiting.**
- Group by *full* IP rather than /24. On a public instance the /24 of a public IP groups unrelated households at the same ISP together — a privacy/UX smell. On a LAN, /24 vs full IP behaves the same for typical home networks. Full-IP grouping may be strictly better for the public case.
- An optional user-entered room code for cross-network sharing.

---

## D5. IPv6 connections are rejected — first-pass

**Decision.** `subnet_key()` returns `None` for IPv6 and the WebSocket handler closes the connection with a warning.

**Why.** Kept the first version simple.

**Should revisit.** Many mobile networks and some ISPs are IPv6-first. A key derived from the IPv6 prefix (e.g. /64) would be the equivalent of the /24 grouping. Note that in practice IPv6 clients never reach this check: the server binds `0.0.0.0`, which listens on IPv4 only, so IPv6 connections (including `[::1]`, IPv6 localhost) are refused before they reach the handler. Supporting IPv6 means changing the bind address as well as `subnet_key()`.

---

## D6. `Arc<tokio::sync::Mutex<HashMap<subnet, HashMap<Uuid, Peer>>>>` for the peer registry — first-pass

**Decision.** One shared map, nested by subnet then by peer ID, guarded by tokio's async Mutex, shared via `Arc` through axum `State`.

**Why.** Straightforward and easy to reason about. The nesting makes "all peers on my subnet" a single lookup.

**Alternatives.** `std::sync::Mutex` is cheaper and fine as long as the lock is never held across an `.await` (currently true). `DashMap` or `RwLock` if contention ever matters. It won't at this project's scale — don't change this without a reason.

**One lock per step.** A peer is removed and the peers left to tell are read in the same lock block, so every other peer either heard of the leaver and hears it left, or never heard of it. The join path works the same way. From `docs/specs/2026-11-17-peer-left-broadcast.md`.

---

## D7. Per-peer `mpsc` channel (capacity 32) as the outbound path — first-pass

**Decision.** Each connection gets an `mpsc::channel::<SignalingMessage>(32)`. The `Sender` lives in `Peer` so other tasks can push messages to that peer; the connection's own task drains the `Receiver` into the WebSocket.

**Why.** Standard actor-ish pattern: only the connection's own task writes to its socket, everyone else goes through the channel. Avoids sharing the socket sink between tasks.

**Full inboxes (presence messages).** `peer_joined` and `peer_left` are sent with `try_send`. If a peer's inbox is full, its copy is dropped and logged at warn; nobody waits. Waiting let two connection tasks block on each other forever, and let one stuck peer stall every newcomer's `welcome`. A dropped message leaves that peer's list stale until it reconnects. See `docs/specs/2026-11-17-peer-left-broadcast.md`.

**Socket writes** time out after 10 seconds, after which the connection ends and the normal cleanup runs. This only catches a peer that stays connected but stops reading: writes to a peer that vanished usually succeed into the OS send buffer. Vanished peers need a heartbeat, not built yet.

**Notes.** 32 is arbitrary. Relay messages (`offer`, `answer`, `ice_candidate`) are still open: decide when implementing the relay.

---

## D8. Wire format: JSON, `#[serde(tag = "type", rename_all = "snake_case")]` — settled

**Decision.** `SignalingMessage` serializes as `{"type": "offer", "from": ..., "to": ..., "sdp": ...}` etc.

**Why.** Internally tagged enums are the most natural shape for a JS client to switch on. `snake_case` keeps the JSON conventional.

**Current variants.** `peer_joined`, `peer_left`, `offer`, `answer`, `ice_candidate`. Expect to add something like `welcome` (your own ID + current peer list) and maybe `ping`/`pong`.

---

## D9. Privacy and logging policy — settled

**Decision.** Never persist transferred content on the server. Logging is encouraged for diagnosing problems, with a hard line on what's logged:

- **OK:** connection/disconnection events, peer IDs, subnet keys, message *types*, counts, errors.
- **Never:** SDP or ICE candidate payloads, file names or sizes, text content, anything from a DataChannel.

**Why.** SDP and ICE contain local IPs and network topology; content is the user's. The public instance must be trustworthy without the user having to take anyone's word for it.

**Message types in logs** come from `SignalingMessage::type_name()`, never from `{:?}`: `Debug` on `Offer`, `Answer` or `IceCandidate` would print the SDP or candidate. From `docs/specs/2026-11-17-peer-left-broadcast.md`.

---

## D10. Shared `crabshare-core` crate, compiled to WebAssembly for the browser — settled

**Context.** Files go browser to browser and never reach the server (D1), so hashing, chunking and verifying chunks have to run in the browser. The DataChannel already protects data in transit, but not against chunking or reassembly bugs at either end, and it can't resume a transfer once the channel closes.

**Decision.**
- Turn the repo into a Cargo workspace with two crates: `server` (the signaling server) and `crabshare-core` (content hashing, Merkle tree, chunk format and verification).
- `crabshare-core` is plain Rust with no async runtime or server dependencies (no tokio, no axum), so it builds for both native and `wasm32` targets.
- `crabshare-core` is compiled to WebAssembly and called from the frontend JavaScript through `wasm-bindgen`. Signaling and WebRTC setup stay in JavaScript in the browser; Wasm is used only for file work.
- File hashes and Merkle roots travel only over the DataChannel, never through signaling. A file's hash identifies the file, so the server seeing it would reveal who shares what.

**Why.** One implementation of the verification logic, tested natively with `cargo test`, and fast enough in the browser for large files. A Merkle tree over chunks lets a receiver verify each chunk as it arrives and resume an interrupted transfer by proving which chunks it already has.

**Timing.** The workspace split happens when work on `crabshare-core` starts, after M2. The Wasm build is needed from M6 (file sharing). Nothing changes before then.

**Name.** `crabshare-core` is a working name. A plain `core` would clash with Rust's built-in `core` library and is reserved on crates.io. The crate may get a standalone name before it is published.

---

## D11. Work goes to `develop`; `main` takes releases - settled

**Decision.**
- `develop` is GitHub's default branch and takes all work. The owner pushes to it directly. Outside contributors open pull requests to it.
- `main` takes only releases: a pull request from `develop` to `main`, merged by the owner with a merge commit. A push to `main` is what deploys the landing page (D13), so merging an outside change never makes it live by itself.
- `main` never gets a change that isn't on `develop` first. An urgent fix goes to `develop`, then is released.
- Agents never commit or push.

**Why.** Outside pull requests need automatic checks and a step between "merged" and "live". The owner keeps pushing straight to `develop` because GitHub doesn't let an author approve their own pull request, so for the owner's own work a pull request adds a step without adding a reviewer. Releases are merge commits because a squash or rebase merge rewrites `develop`'s commits into new ones on `main`, and the two branches drift until releases conflict.

**Rulesets.** A repository admin bypass skips every rule in its ruleset, so rules that must hold for the owner too sit in rulesets with no bypass.
- `protect_develop` (`develop`): a pull request with one approval and code-owner review (`.github/CODEOWNERS`), the extra approval for unattributed changes, stale approvals dismissed on a new push, approval of the most recent push, squash merges only, and the `checks` status from GitHub Actions on an up-to-date branch. Admin bypass "always".
- `protect_main` (`main`): restrict updates, so only the bypass list can change `main` and no outside pull request can be merged into it; a pull request with one approval and code-owner review, and the extra approval for unattributed changes; merge commits only. Admin bypass "always".
- `branch_history` (`develop` and `main`): no deletion, no force push. No bypass.
- `release_checks` (`main`): the `checks` status from GitHub Actions. No bypass.

**CI.** `.github/workflows/ci.yml` runs the checks (`cargo fmt --check`, clippy and the tests with `--locked`, `scripts/doc-refs-check.py`) on every pull request and every push to `develop` and `main`. It uses the `pull_request` trigger, a read-only token, no secrets, and no third-party actions; its one action is pinned to a commit. `rust-toolchain.toml` pins the Rust version for CI and every developer alike. A pull request runs its own copy of the checks, so one that touches them is read before its green check counts.

**Rejected.** Outside pull requests straight into `main`: merging would deploy. A pull request for each of the owner's own changes: revisit as the repository grows. Squash, rebase or fast-forward releases: the first two drift, the last leaves no record of a release. Git-flow's release and hotfix branches: more branches than one maintainer needs.

**Consequences.** The review scripts compare against where the current branch left the default branch (`git merge-base origin/HEAD HEAD`). The review loop's record goes in the maintainer's notes. Spec: `docs/specs/2026-11-24_develop-branch-flow.md`.

---

## D12. System cards and a review loop - settled

**Decision.** Each part of the app, cut by how it fails, has a card in `docs/systems/` with its files, invariants, tripwires and a setting for how involved the owner is. After a tier 2 or 3 change, `/review-loop` runs a general reviewer and one reviewer per touched card, then a separate fixer, then one recheck. Specs live in `docs/specs/` only while their change is in progress.

**Why.** The author of a change is the worst judge of it. Cutting by failure mode puts the right questions to each change: a log line goes to the privacy card, a new lock to state and timing. Tripwires catch risky code outside a card's own files.

**Rejected.** One general review only: it spreads its attention thin and misses system-specific invariants. Cards by layer (server, frontend): a privacy leak or a race can sit in either.

**Origin.** Built from version 0.2.7 of the coding agents harness (`.harness-version`).

---

## D13. The landing page is deployed to Cloudflare Pages by GitHub Actions - settled

**Decision.** `.github/workflows/deploy-landing.yml` publishes `landing/` as it is (no build step) to the Cloudflare Pages Direct Upload project `crabshare-landing` on every push to `main` that changes `landing/`, and on manual dispatch. It authenticates with a Cloudflare API token that has only the Account > Cloudflare Pages > Edit permission, held as a secret of the GitHub Environment `production`, which only `main` can use. The workflow's own GitHub token is read-only, and the action and `wrangler` are pinned to exact versions. The project's custom domains serve the page. The signaling server isn't deployed yet.

**Why.** GitHub pushes to Cloudflare, so Cloudflare holds no access to this repository. The token can deploy pages but can't touch DNS, other Cloudflare products or GitHub. Pins mean a new release of the action or of `wrangler` itself reaches the token only after review. `wrangler`'s own npm dependencies still resolve at run time with no lockfile, so the token's Pages-only scope is what limits them.

**Rejected.** The Cloudflare GitHub app, connected to a Git-integrated Pages project: Cloudflare pulls from the repository, and the app asks for write access to code and settings. Keeping the Git-integrated project with its builds turned off and deploying into it with `wrangler`: Cloudflare doesn't allow a Git-integrated project to become a Direct Upload one, and its docs don't say that deploys keep working once the app is removed. A `deployments: write` permission so the action records GitHub deployments: not needed. Spec: `docs/specs/2026-11-24_landing-deploy-action.md`.

**Releases.** A change reaches `main` only through a checked release pull request from `develop` (D11), so only a release deploys. Spec: `docs/specs/2026-11-24_develop-branch-flow.md`.

**Revisit.** When anyone besides the owner can merge to `main`, add required reviewers to the `production` environment, so each deploy waits for the owner's approval.
