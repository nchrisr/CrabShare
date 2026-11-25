# Origin and Host checks on `/ws`

**Tier:** 3. It changes who the server accepts as a peer (tier 3 item 2: how a peer's identity is trusted).
**Systems:** peer-identity-and-routing (teach-me; tripwire: changes the `/ws` entry point), privacy-and-logging (teach-me; tripwire: a new log line next to client headers), resource-limits (keep-me-posted; refusals are logged), frontend (trust; the page explains a refused address).
**Why:** `/ws` accepts any WebSocket upgrade. A web page the user visits on another site runs in their browser, inside their LAN, and can open `ws://<LAN IP>:3000/ws`. The server then puts it in the user's subnet group and sends it every peer ID in `welcome`; once relaying exists it would receive offers too. Checked on the running server on 2026-11-25: an upgrade with `Origin: https://evil.example` and `Host: evil.example:3000` gets `101 Switching Protocols`. A plain `Origin == Host` check is not enough either: with DNS rebinding the attacker's own name resolves to the LAN address, so both headers carry the attacker's name and match. This affects every self-hosted user today and the public instance later.

## Decisions

### 1. The check runs in the `/ws` handler, before the upgrade, as a pure function
- **Options:** a tower middleware layer on the `/ws` route; a check inside `on_upgrade` after the 101; a check in the handler before `on_upgrade`.
- **Chosen:** in the handler, before `on_upgrade`, calling a pure function in a new `src/origin.rs`. A refused client never gets a socket, a subnet group or a peer ID. A pure function over header values is unit-testable without a server. A middleware layer would hide the check away from the one route it guards.
- **Still uncertain:** nothing.

### 2. No `Origin` header: allow
- **Options:** require `Origin` from everyone; allow when absent.
- **Chosen:** allow when absent. Every browser context (pages, workers, sandboxed frames, `data:` URLs) sends `Origin` on WebSocket requests, and scripts can't remove or change it, so every attack in scope carries one. Tools (`websocat`) and future native clients usually send none, and the project designs for native clients. A non-browser client on the LAN can already connect directly; this check is not meant to stop it. An `Origin` header that is present but empty or unreadable counts as present, never as missing.
- **Still uncertain:** nothing.

### 3. With `Origin`, the `Host` must be one a real visit would use
- **Options:** any `Host`; an IP literal or `localhost` only; that plus names ending in `.local`; that plus a configured list.
- **Chosen:** the host part of `Host` (port removed), compared ASCII case-insensitively, must be: an IPv4 literal, a bracketed IPv6 literal, exactly `localhost`, or a name ending in `.local` with at least one label before it. This stops DNS rebinding: the attack only works while the browser uses the attacker's name, so `Host` carries that name. Browsers rewrite unusual IPv4 forms (`127.1`, octal, hex) to dotted decimal before sending, so a strict IP parse is enough. `.local` names are normally resolved by multicast DNS inside the LAN, so only the network's DNS operator or someone on the LAN can point one at an address, and both can reach the server directly anyway. Allowing `.local` means a Raspberry Pi reached as `pi.local` works with no setup.
- **Still uncertain:** some resolvers send `.local` lookups to ordinary DNS with a search suffix appended; no practical attack by an internet attacker was found. Locked out, by design until M7: bare hostnames (`raspberrypi`), `.lan`, `.home.arpa`, router names, VPN names, `foo.localhost`, a trailing dot.

### 4. With `Origin`, it must name the same server as `Host`
- **Options:** compare host only; compare host and port; compare the full origin including scheme against a reconstructed one.
- **Chosen:** `Origin` must be `http://<authority>` or `https://<authority>` (scheme case-insensitive) with no path, and `<authority>` must equal the `Host` value, ASCII case-insensitively. Chrome, Firefox and Safari all write both without the default port, so `http://pi.local` matches `pi.local` and `http://192.168.1.20:3000` matches `192.168.1.20:3000`. The page builds its socket URL from `location.host` (`static/app.js`), so its own connection always matches; observed in the running page: origin `http://localhost:3000`, host `localhost:3000`. This stops the simple attack: a script from another site connecting straight to the LAN address. Another service on the same IP uses a different port, so it fails too. `Origin: null` (sandboxed frames, `file://` pages, some app shells) and anything unparseable fail.
- **Still uncertain:** nothing for direct connections.

### 5. No allow-list setting yet
- **Options:** add an environment variable now; keep everything as constants and add the setting at M7.
- **Chosen:** constants now, setting at M7. The project has no configuration surface yet, and M7 must design one for the proxy anyway. Consequences, written into `docs/DECISIONS.md`:
  - Until M7, browsers are accepted only at an IP literal, `localhost` or a `.local` name, **with or without a proxy**. Any real domain name is refused, so is a self-hoster's custom name with TLS, and so are app shells that send their own origin (`tauri://`, `capacitor://`, `null`).
  - M7 requirement: the setting must list allowed **origins**, not only hosts, and still require `Origin` to match one of them. A host-only list would bring back the cross-site attack on the public instance.
- **Still uncertain:** how many self-hosters use custom names. If one reports it before M7, the setting can come forward as its own change.

### 6. `Host` comes from the `Host` header only
- **Options:** the `Host` header only; the header, else the request URI's authority (HTTP/2).
- **Chosen:** the header only. The route takes `GET`; WebSockets over HTTP/2 use extended `CONNECT`, which this route doesn't accept, and the server doesn't speak HTTP/2 to browsers anyway, so a fallback could never run. `Origin` present with no `Host`, more than one `Host` or `Origin` header, or a value that isn't visible ASCII: refused as malformed.
- **Still uncertain:** nothing.

### 7. The response and the log line
- **Options:** close after upgrading; respond 403 before upgrading; respond 404 to hide the route. For logging: one line per refusal; a bounded summary.
- **Chosen:** `403 Forbidden` with no body, before the upgrade. The route is public in the page source, so hiding it gains nothing. Logging: a process-wide counter per refusal reason, and an `info` line only when that counter reaches 1, 2, 4, 8 and so on: `"/ws upgrade refused: {reason} ({count} so far)"`. A hostile tab retrying in a loop then writes about 20 lines per million attempts instead of a million, with no clock and no extra lock. `reason` is a fixed label (`host not allowed`, `origin mismatch`, `malformed headers`). No header values and no address: they are client data (D9), and an attacker's domain in the log would be text the attacker chose.
- **Still uncertain:** nothing; real rate limits on connections are still M7's abuse limits.

### 8. The page explains a refused address instead of retrying forever
- **Options:** leave the page as is; check the address in the page too.
- **Chosen:** a browser can't read a 403 on a WebSocket handshake, so today a refused user sees "Reconnecting…" forever, and each retry adds a refusal. Before opening the socket, the page checks `location.hostname` against the same rule as decision 3. If it fails, it doesn't connect and shows: open CrabShare by IP address, `localhost` or a `.local` name.
- **Still uncertain:** the page's rule and the server's rule are two copies; the unit tests pin the server's, and the by-hand check pins the page's.

### 9. Nothing untrusted may run in CrabShare's own origin
- **Options:** leave it implicit; record it as an invariant now.
- **Chosen:** record it. After this change the check reduces to "the script runs in CrabShare's origin", which holds only while nothing untrusted runs there. At M6, an HTML file received from a peer and opened as a `blob:` URL would run as CrabShare and could open `/ws` with a valid `Origin`. Invariant: received content is never rendered in the app's origin (download only, or a sandboxed opaque origin), and text from peers is set as text, never as HTML. True today: `static/` has no `innerHTML`.
- **Still uncertain:** nothing now; M6's spec must honour it.

## How it could go wrong

**peer-identity-and-routing**
- *Real users locked out.* A legitimate address form produces a `Host` or `Origin` the rules refuse. Caught by: unit tests over the address forms below, the two-tab check on `localhost` and on the LAN IP, a `.local` check with the Mac's own name, and decision 8's message making the cause visible.
- *A bypass through string handling.* A prefix or suffix match lets `192.168.1.20.evil.com` or `xlocal` through. Caught by: parsing the host part as an IP address rather than matching text, splitting the port off once from the right (bracket-aware), and a unit test for each hostile form.
- *The check is skipped.* A later change adds another WebSocket route or moves the upgrade without the check. Caught by: the card's tripwire on `.route(` and `on_upgrade(`, and the new invariant. The unit tests don't cover the wiring; the curl probe does, by hand.

**privacy-and-logging**
- *Header values leak into logs.* A later change logs the refused `Origin` or `Host`. Caught by: the card's watch pattern, the new invariant, and `reason` being a fixed enum label.
- *Refusals become a tracking record.* Logging the address would record who visited which hostile page. Caught by: decision 7 logs no address; the reviewer checks it.

## Changes

- **`src/origin.rs` (new).**
  - `pub enum RefusalReason { HostNotAllowed, OriginMismatch, MalformedHeaders }` with `pub fn label(&self) -> &'static str`.
  - `pub fn check_upgrade_origin(headers: &HeaderMap) -> Result<(), RefusalReason>`: decisions 2, 3, 4 and 6.
  - `pub struct RefusalCounts` with `pub fn record(&self, reason: RefusalReason) -> Option<u64>`: bumps that reason's atomic counter; returns the new count when it is a power of two (log it), else `None`. A `pub static REFUSALS` holds the server's counts; tests use their own instance.
  - Private helpers: `fn is_allowed_host(host: &str) -> bool`, `fn split_host_port(authority: &str) -> Option<(&str, Option<u16>)>` (bracket-aware; empty, non-numeric or out-of-range port is `None`), `fn origin_authority(origin: &str) -> Option<&str>`.
  - Unit tests in the same file.
- **`src/main.rs`.** `mod origin;`. `ws_handler` returns `Response`. First thing in the handler: `check_upgrade_origin(&headers)`; on `Err(reason)`, call `REFUSALS.record`, log if it returns a count, return `StatusCode::FORBIDDEN`. On `Ok` the rest is unchanged.
- **`static/app.js`.** Before creating the WebSocket, check `location.hostname` (decision 8); on failure, show the message in the connection status area and don't connect or retry.
- **`docs/DECISIONS.md`.** New D15, "Browser connections must come from the server's own page": decisions 2 to 5, 7 and 9, why `Host` is checked (DNS rebinding), what is locked out until M7, and the M7 requirement on origins. A line under D3's proxy consequence pointing at D15.
- **`docs/systems/peer-identity-and-routing.md`.** Owns `src/origin.rs`. Invariant: a `/ws` request with an `Origin` header is upgraded only if `Host` is allowed and `Origin` names the same authority; refused requests never get a peer ID. Checks: unit tests in `src/origin.rs` (not the wiring; see the curl probe).
- **`docs/systems/privacy-and-logging.md`.** Invariant: refusal logs carry a fixed reason label and a count only.
- **`docs/systems/frontend.md`.** Invariant from decision 9.
- **`AGENTS.md`.** One line in "Where things live": `src/origin.rs  check_upgrade_origin(): which browser pages may open /ws`.
- **`README.md`.** If it has a self-hosting section: reach the server by IP address, `localhost` or a `.local` name; reverse proxies and custom names aren't supported yet.

## Testing

- **Unit tests (`src/origin.rs`).**
  - Allowed: no `Origin` (any or no `Host`); `localhost:3000` with `http://localhost:3000`; `localhost` with `http://localhost`; `192.168.1.20:3000` with `http://192.168.1.20:3000`; `pi.local` with `http://pi.local`; mixed case; `HTTP://` scheme; `[::1]:3000` with `http://[::1]:3000`; `https://` origin with a matching host; `0.0.0.0:3000` with a matching origin (an IP literal, harmless).
  - Host not allowed: `evil.com:3000` with `http://evil.com:3000` (rebinding); `192.168.1.20.evil.com`; `xlocal`; `.local`; `pi.local.`; `localhost.evil.com`; `foo.localhost`.
  - Origin mismatch: `192.168.1.20:3000` with `https://evil.com`; port differs; `http://192.168.1.20:80` with `192.168.1.20`; `null`; origin with a path.
  - Malformed: empty `Origin`; two `Origin` headers; two `Host` headers; non-ASCII `Origin`; `Origin` with no `Host`; `localhost:`, `localhost:abc`, `localhost:99999`; `user@host` in `Host`.
  - `RefusalCounts::record`: returns counts 1, 2, 4, 8 and `None` in between, per reason.
- **Against the running server (`cargo run`, `curl`).** Foreign `Origin` and `Host` gives 403; matching `localhost` gives 101; no `Origin` gives 101. Twenty refusals in a row give five log lines (1, 2, 4, 8, 16), each with only the label and count.
- **By hand.** Two tabs on `http://localhost:3000` see each other. A phone or second tab on `http://<LAN IP>:3000` joins. `http://<this Mac's name>.local:3000` joins. `http://127.0.0.1.nip.io:3000`-style or bare-hostname access shows decision 8's message and stops retrying. `websocat ws://127.0.0.1:3000/ws` still connects.
- **Can't be checked here.** A real DNS rebinding attack (needs a domain with a short-lived record); behaviour behind a reverse proxy (M7); Chrome's local network prompt.

## Owner steps

1. Approve the spec, one decision at a time.
2. After the build and `/review-loop`: read the briefing for the teach-me cards, review the diff on the branch Claude created, commit and push to `develop`.
3. Optionally run the two-tab and phone check yourself first.
4. This spec stays out of the repo while the hole is open. It goes into `docs/specs/` in the same commit as the fix.
