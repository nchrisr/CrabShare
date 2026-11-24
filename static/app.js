"use strict";

// Live picture of this network: peers on a ring, the CrabShare server
// outside it, and every message that moves drawn as it happens. Kept in sync
// with the server's welcome / peer_joined / peer_left messages.
//
// Every string from the server or a peer goes into the page with
// textContent or setAttribute, never as HTML.

const ADJECTIVES = [
    "Amber", "Brave", "Calm", "Dusty", "Eager", "Fuzzy", "Gentle", "Happy",
    "Icy", "Jolly", "Kind", "Lucky", "Mellow", "Nimble", "Olive", "Plucky",
];
const ANIMALS = [
    "Otter", "Heron", "Badger", "Falcon", "Gecko", "Lynx", "Marmot", "Newt",
    "Owl", "Panda", "Quokka", "Robin", "Seal", "Tapir", "Wombat", "Yak",
];

// Device kinds the server may send, mapped to an icon and a label.
// Anything not listed shows as unknown.
const DEVICE_TABLE = {
    iphone: { icon: "phone", label: "iPhone" },
    android: { icon: "phone", label: "Android" },
    ipad: { icon: "tablet", label: "iPad" },
    mac: { icon: "computer", label: "Mac" },
    windows: { icon: "computer", label: "Windows" },
    linux: { icon: "computer", label: "Linux" },
};

// Stage geometry, in the SVG's 0..100 units. Peers sit on the ring; the
// lane around it is the signaling path, which reaches the server at the top.
const CENTER_X = 50;
const CENTER_Y = 56;
// 28 keeps the "You" node's two-line label inside the stage down to 360 px
// wide, where labels hide.
const RING_RADIUS = 28;
const LANE_RADIUS = 40;
const SERVER = { x: 50, y: 7 };
const LANE_TOP = { x: 50, y: CENTER_Y - LANE_RADIUS };

const CROWDED_AT = 13;
const MAX_PACKETS = 40;
const MAX_FLASHES = 40;
const LEG_MS = 1100;
const FEED_LIMIT = 20;
const SVG_NS = "http://www.w3.org/2000/svg";

const statusEl = document.getElementById("status");
const stageEl = document.getElementById("stage");
const wiresEl = document.getElementById("wires");
const nodesEl = document.getElementById("nodes");
const peersEl = document.getElementById("peers");
const emptyEl = document.getElementById("empty");
const feedEl = document.getElementById("feed");
const signalStateEl = document.getElementById("signal-state");
const signalEndpointEl = document.getElementById("signal-endpoint");
const signalSecurityEl = document.getElementById("signal-security");
const contentPeerEl = document.getElementById("content-peer");
const secureContextEl = document.getElementById("secure-context");

const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

let myId = null;
let myDevice = null;
let myName = null;
// Peer ID -> { device, name }. Both are null until the server sends them.
let peers = new Map();
let selectedPeer = null;
let positions = new Map();
let activePackets = 0;
let activeFlashes = 0;
let retryDelayMs = 1000;

// Node elements on the ring, by peer ID, plus "server".
const nodeEls = new Map();
// Open data streams by "from>to", so repeated progress updates reuse one chord.
const dataStreams = new Map();

// ---------------------------------------------------------------------------
// Pure helpers

// A readable name derived from the peer ID, so every device shows the same
// name for the same peer without the server sending one.
function nameFor(id) {
    const hex = id.replace(/-/g, "");
    const adjective = ADJECTIVES[parseInt(hex.slice(0, 2), 16) % ADJECTIVES.length];
    const animal = ANIMALS[parseInt(hex.slice(2, 4), 16) % ANIMALS.length];
    return `${adjective} ${animal}`;
}

// The table entry for a device kind, or null. hasOwn keeps inherited keys
// such as "constructor" from counting as entries.
function deviceEntry(device) {
    return Object.hasOwn(DEVICE_TABLE, device) ? DEVICE_TABLE[device] : null;
}

function iconFor(device) {
    const entry = deviceEntry(device);
    return entry ? entry.icon : "unknown";
}

function deviceLabel(device) {
    const entry = deviceEntry(device);
    return entry ? entry.label : "Unknown device";
}

// A string field from a message, or null if it is missing or not a string.
function stringOrNull(value) {
    return typeof value === "string" ? value : null;
}

// A name from a message, or null if it is missing or blank, so a blank name
// falls back to the generated one everywhere.
function nameOrNull(value) {
    if (typeof value !== "string") return null;
    const trimmed = value.trim();
    return trimmed === "" ? null : trimmed;
}

// This browser's own guess at its device kind. Used only for this device's
// icon until the server sends its kind, and never sent anywhere.
function guessOwnDevice() {
    const agent = navigator.userAgent;
    if (agent.includes("iPad")) return "ipad";
    if (agent.includes("iPhone")) return "iphone";
    if (agent.includes("Android")) return "android";
    if (agent.includes("Macintosh")) {
        // iPads in desktop mode say Macintosh but have a touch screen.
        return navigator.maxTouchPoints > 1 ? "ipad" : "mac";
    }
    if (agent.includes("Windows")) return "windows";
    if (agent.includes("Linux")) return "linux";
    return null;
}

// Positions on the ring as SVG units: this device at the bottom, the others
// spread evenly clockwise from it in ID order, so joins don't reshuffle.
function layoutRing(selfId, otherIds) {
    const layout = new Map();
    const ids = selfId ? [selfId, ...otherIds] : [...otherIds];
    const count = ids.length;
    for (let index = 0; index < count; index++) {
        const angle = Math.PI / 2 + (index * 2 * Math.PI) / count;
        layout.set(ids[index], {
            angle: angle,
            x: CENTER_X + RING_RADIUS * Math.cos(angle),
            y: CENTER_Y + RING_RADIUS * Math.sin(angle),
        });
    }
    return layout;
}

// Plain words for an ICE candidate type from RTCPeerConnection.getStats().
function describeCandidateType(type) {
    switch (type) {
        case "host":
            return "Same network, direct";
        case "srflx":
        case "prflx":
            return "Direct, through your router's public address";
        case "relay":
            return "Through a relay server";
        default:
            return "Unknown";
    }
}

// SVG path from a peer on the ring out to the lane, around the lane to the
// top, then up to the server. Signaling never crosses the ring.
function signalPath(point) {
    const laneX = CENTER_X + LANE_RADIUS * Math.cos(point.angle);
    const laneY = CENTER_Y + LANE_RADIUS * Math.sin(point.angle);

    // Turn from the peer's angle to the top (-90 degrees) the short way.
    let delta = -Math.PI / 2 - point.angle;
    while (delta <= -Math.PI) delta += 2 * Math.PI;
    while (delta > Math.PI) delta -= 2 * Math.PI;
    const sweep = delta > 0 ? 1 : 0;

    return [
        `M${point.x} ${point.y}`,
        `L${laneX} ${laneY}`,
        `A${LANE_RADIUS} ${LANE_RADIUS} 0 0 ${sweep} ${LANE_TOP.x} ${LANE_TOP.y}`,
        `L${SERVER.x} ${SERVER.y}`,
    ].join(" ");
}

function chordPath(from, to) {
    return `M${from.x} ${from.y} L${to.x} ${to.y}`;
}

// ---------------------------------------------------------------------------
// Drawing the ring

function displayName(id) {
    if (id === myId) return myName ?? nameFor(id);
    return peers.get(id)?.name ?? nameFor(id);
}

function deviceOf(id) {
    if (id === myId) return myDevice ?? guessOwnDevice();
    return peers.get(id)?.device ?? null;
}

function makeIcon(icon) {
    const svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("class", "icon");
    svg.setAttribute("aria-hidden", "true");
    const use = document.createElementNS(SVG_NS, "use");
    use.setAttribute("href", `#icon-${icon}`);
    svg.append(use);
    return svg;
}

// The typed name on top, and the generated name below it, so a copied name
// can still be told apart.
function makeLabel(id) {
    const label = document.createElement("span");
    label.className = "node-label";
    const name = document.createElement("span");
    name.className = "node-name";
    name.textContent = displayName(id);
    label.append(name);

    const typed = id === myId ? myName : peers.get(id)?.name;
    const sub = document.createElement("span");
    sub.className = "node-sub";
    if (id === myId) {
        sub.textContent = typed ? `You · ${nameFor(id)}` : "You";
    } else {
        sub.textContent = typed ? nameFor(id) : id.slice(0, 8);
    }
    label.append(sub);
    return label;
}

function makeServerNode() {
    const node = document.createElement("div");
    node.className = "node server";
    node.style.left = `${SERVER.x}%`;
    node.style.top = `${SERVER.y}%`;
    node.title = "The CrabShare server introduces devices. Content never goes through it.";
    node.append(makeIcon("server"));
    const label = document.createElement("span");
    label.className = "node-label";
    const name = document.createElement("span");
    name.className = "node-name";
    name.textContent = "CrabShare server";
    const sub = document.createElement("span");
    sub.className = "node-sub";
    sub.textContent = "signaling only";
    label.append(name, sub);
    node.append(label);
    return node;
}

function makePeerNode(id) {
    const isMe = id === myId;
    const node = document.createElement(isMe ? "div" : "button");
    node.className = isMe ? "node me entering" : "node entering";
    if (!isMe) {
        node.type = "button";
        node.addEventListener("click", () => {
            selectedPeer = selectedPeer === id ? null : id;
            render();
        });
    }
    // Two frames: the first gives the new node a computed "entering" style,
    // so removing the class in the second starts the fade-in transition.
    requestAnimationFrame(() => {
        requestAnimationFrame(() => node.classList.remove("entering"));
    });
    return node;
}

function fillPeerNode(node, id) {
    const device = deviceOf(id);
    node.replaceChildren(makeIcon(iconFor(device)), makeLabel(id));
    node.setAttribute(
        "aria-label",
        `${displayName(id)}, ${deviceLabel(device)}${id === myId ? ", this device" : ""}`,
    );
    if (id !== myId) {
        node.setAttribute("aria-pressed", String(selectedPeer === id));
        node.classList.toggle("selected", selectedPeer === id);
    }
}

function renderRing() {
    if (!nodeEls.has("server")) {
        const server = makeServerNode();
        nodeEls.set("server", server);
        nodesEl.append(server);
    }
    nodeEls.get("server").classList.toggle("offline", myId === null);

    const otherIds = [...peers.keys()].sort();
    positions = layoutRing(myId, otherIds);
    stageEl.classList.toggle("crowded", positions.size >= CROWDED_AT);

    for (const [id, point] of positions) {
        let node = nodeEls.get(id);
        if (!node) {
            node = makePeerNode(id);
            nodeEls.set(id, node);
            nodesEl.append(node);
        }
        node.style.left = `${point.x}%`;
        node.style.top = `${point.y}%`;
        fillPeerNode(node, id);
    }

    // Fade out nodes that are no longer on the network.
    for (const [id, node] of nodeEls) {
        if (id === "server" || positions.has(id)) continue;
        nodeEls.delete(id);
        node.classList.add("leaving");
        node.disabled = true;
        setTimeout(() => node.remove(), 700);
    }

    redrawStreams();
}

// ---------------------------------------------------------------------------
// Lists and panel

function makeListItem(id) {
    const isMe = id === myId;
    const device = deviceOf(id);
    const item = document.createElement("li");
    item.classList.toggle("selected", selectedPeer === id);
    item.append(makeIcon(iconFor(device)));

    const text = document.createElement("span");
    text.className = "peer-text";
    const name = document.createElement("span");
    name.className = "peer-name";
    name.textContent = displayName(id);
    const detail = document.createElement("span");
    detail.className = "peer-detail";
    const typed = isMe ? myName : peers.get(id)?.name;
    let detailText = deviceLabel(device);
    if (typed) detailText += ` · ${nameFor(id)}`;
    if (isMe) detailText += " · this device";
    detail.textContent = detailText;
    text.append(name, detail);

    const shortId = document.createElement("span");
    shortId.className = "id";
    shortId.textContent = id.slice(0, 8);
    item.append(text, shortId);
    return item;
}

// This device first, so its name is on screen even when ring labels are
// hidden (narrow screens, crowded ring); then the others by ID.
function renderList() {
    peersEl.replaceChildren();
    if (myId) peersEl.append(makeListItem(myId));
    for (const id of [...peers.keys()].sort()) {
        peersEl.append(makeListItem(id));
    }
    emptyEl.hidden = peers.size > 0;
}

function renderPanel() {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    signalEndpointEl.textContent = `${scheme}://${location.host}/ws`;
    signalSecurityEl.textContent =
        scheme === "wss"
            ? "Encrypted with TLS."
            : "Not encrypted: plain WebSocket on the local network. It carries only introductions, never your content.";

    if (selectedPeer && peers.has(selectedPeer)) {
        contentPeerEl.textContent = `With ${displayName(selectedPeer)}: no direct connection yet.`;
    } else {
        contentPeerEl.textContent = "Tap a device on the ring to see how you connect to it.";
    }

    const webrtc = typeof RTCPeerConnection === "function";
    secureContextEl.textContent =
        `Secure context: ${window.isSecureContext ? "yes" : "no"}. ` +
        `WebRTC available in this browser: ${webrtc ? "yes" : "no"}.`;
}

function render() {
    if (selectedPeer && !peers.has(selectedPeer)) selectedPeer = null;
    renderRing();
    renderList();
    renderPanel();
}

function setStatus(state, text) {
    statusEl.dataset.state = state;
    statusEl.textContent = text;
    signalStateEl.dataset.state = state;
    signalStateEl.textContent = state === "connected" ? "Connected" : text.replace("…", "");
}

function addFeed(type, id) {
    const item = document.createElement("li");
    const time = document.createElement("time");
    time.textContent = new Date().toLocaleTimeString([], { hour12: false });
    const kind = document.createElement("code");
    kind.textContent = type;
    item.append(time, kind);
    if (id) {
        const who = document.createElement("span");
        who.textContent = id === myId ? "you" : displayName(id);
        item.append(who);
    }
    feedEl.prepend(item);
    while (feedEl.children.length > FEED_LIMIT) feedEl.lastElementChild.remove();
}

// ---------------------------------------------------------------------------
// Traffic animations

function addPath(d, className) {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    path.setAttribute("class", className);
    wiresEl.append(path);
    return path;
}

function addCircle(radius, className) {
    const circle = document.createElementNS(SVG_NS, "circle");
    circle.setAttribute("cx", CENTER_X);
    circle.setAttribute("cy", CENTER_Y);
    circle.setAttribute("r", radius);
    circle.setAttribute("class", className);
    wiresEl.append(circle);
}

// The fixed backdrop: the lane, the ring, and the stub from the lane up to
// the server. Drawn from the same constants nodes and packets use, so they
// always line up. The server node covers the stub's top end.
function drawBackdrop() {
    addCircle(LANE_RADIUS, "lane");
    addCircle(RING_RADIUS, "ring");
    addPath(`M${LANE_TOP.x} ${LANE_TOP.y} L${SERVER.x} ${SERVER.y}`, "lane");
}

// Moves one dot along a path. Resolves when it arrives. With reduced motion,
// or when the stage is already busy, the path flashes instead. Past
// MAX_FLASHES the leg is dropped, so a burst can't flood the stage.
function sendPacket(d, kind, reverse) {
    return new Promise((resolve) => {
        if (reducedMotion.matches || activePackets >= MAX_PACKETS) {
            if (activeFlashes >= MAX_FLASHES) {
                resolve();
                return;
            }
            activeFlashes += 1;
            const path = addPath(d, `trail ${kind} flash`);
            setTimeout(() => {
                path.remove();
                activeFlashes -= 1;
                resolve();
            }, LEG_MS);
            return;
        }

        const path = addPath(d, `trail ${kind}`);
        activePackets += 1;
        const dot = document.createElementNS(SVG_NS, "circle");
        dot.setAttribute("r", "1.6");
        dot.setAttribute("class", `packet ${kind}`);
        wiresEl.append(dot);
        const length = path.getTotalLength();
        const start = performance.now();

        function step(now) {
            const t = Math.min((now - start) / LEG_MS, 1);
            const eased = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
            const along = reverse ? 1 - eased : eased;
            const point = path.getPointAtLength(along * length);
            dot.setAttribute("cx", point.x);
            dot.setAttribute("cy", point.y);
            if (t < 1) {
                requestAnimationFrame(step);
                return;
            }
            dot.remove();
            path.remove();
            activePackets -= 1;
            resolve();
        }
        requestAnimationFrame(step);
    });
}

function pulse(id) {
    const node = nodeEls.get(id);
    if (!node) return;
    node.classList.remove("pulse");
    // Reading layout restarts the CSS animation.
    void node.offsetWidth;
    node.classList.add("pulse");
}

// Signal leg between a peer and the server; reverse means server to peer.
function signalLeg(id, towardServer) {
    const point = positions.get(id);
    if (!point) return Promise.resolve();
    return sendPacket(signalPath(point), "signal", !towardServer);
}

function showSignal(event) {
    const recipients = Array.isArray(event.to) ? event.to : [event.to];
    const firstLeg =
        event.from === "server" ? Promise.resolve() : signalLeg(event.from, true);
    firstLeg.then(() => {
        pulse("server");
        for (const id of recipients) {
            if (id === "server") continue;
            signalLeg(id, false).then(() => pulse(id));
        }
    });
}

function closeStream(key, stream) {
    clearTimeout(stream.timer);
    stream.chord.remove();
    stream.text.remove();
    // A newer stream may have taken the key since this one's timer was set.
    if (dataStreams.get(key) === stream) dataStreams.delete(key);
}

// Puts a stream's chord and text where its two peers are now.
function placeStream(stream, from, to) {
    stream.chord.setAttribute("d", chordPath(from, to));
    stream.text.setAttribute("x", (from.x + to.x) / 2);
    stream.text.setAttribute("y", (from.y + to.y) / 2 - 2);
}

// Called after every ring layout: follow moved peers, drop streams whose
// peers are gone (a leave, or the socket closing).
function redrawStreams() {
    for (const [key, stream] of dataStreams) {
        const from = positions.get(stream.fromId);
        const to = positions.get(stream.toId);
        if (from && to) {
            placeStream(stream, from, to);
        } else {
            closeStream(key, stream);
        }
    }
}

function showData(event) {
    const from = positions.get(event.from);
    const to = positions.get(event.to);
    if (!from || !to) return;

    const key = `${event.from}>${event.to}`;
    let stream = dataStreams.get(key);
    if (!stream) {
        const chord = addPath(chordPath(from, to), "chord");
        const text = document.createElementNS(SVG_NS, "text");
        text.setAttribute("class", "progress");
        wiresEl.append(text);
        stream = { fromId: event.from, toId: event.to, chord: chord, text: text, timer: null };
        dataStreams.set(key, stream);
    }
    placeStream(stream, from, to);

    const progress = typeof event.progress === "number" ? event.progress : null;
    const percent =
        progress === null ? "" : `${Math.round(Math.min(Math.max(progress, 0), 1) * 100)}%`;
    const label = stringOrNull(event.label) ?? "";
    stream.text.textContent = [label, percent].filter((part) => part !== "").join(" ");
    sendPacket(chordPath(from, to), "data", false);

    // Close the stream when it finishes, or when updates stop arriving.
    clearTimeout(stream.timer);
    const idleMs = progress !== null && progress >= 1 ? 1500 : 4000;
    stream.timer = setTimeout(() => closeStream(key, stream), idleMs);
}

// The one entry point for everything that moves on the stage.
//   { kind: "signal", type, from, to }  from/to: a peer ID, a list of IDs
//                                       (to only), or "server"
//   { kind: "data", from, to, label, progress }  progress: 0..1 or null
function showTraffic(event) {
    if (event.kind === "signal") {
        showSignal(event);
    } else if (event.kind === "data") {
        showData(event);
    }
}

// ---------------------------------------------------------------------------
// Messages from the server

function handle(message) {
    switch (message.type) {
        case "welcome": {
            if (typeof message.peer_id !== "string" || !Array.isArray(message.peers)) return;
            myId = message.peer_id;
            myDevice = stringOrNull(message.device);
            peers = new Map();
            for (const id of message.peers) {
                if (typeof id === "string") peers.set(id, { device: null, name: null });
            }
            if (Array.isArray(message.peer_details)) {
                for (const detail of message.peer_details) {
                    const entry = peers.get(detail?.peer_id);
                    if (!entry) continue;
                    entry.device = stringOrNull(detail.device);
                    entry.name = nameOrNull(detail.name);
                }
            }
            render();
            addFeed("welcome", myId);
            showTraffic({ kind: "signal", type: "welcome", from: "server", to: myId });
            return;
        }
        case "peer_joined": {
            if (typeof message.peer_id !== "string") return;
            peers.set(message.peer_id, {
                device: stringOrNull(message.device),
                name: nameOrNull(message.name),
            });
            render();
            addFeed("peer_joined", message.peer_id);
            // The server tells every other peer in the group, not only this one.
            const others = [...peers.keys()].filter((id) => id !== message.peer_id);
            showTraffic({
                kind: "signal",
                type: "peer_joined",
                from: message.peer_id,
                to: [myId, ...others],
            });
            return;
        }
        case "peer_left": {
            if (typeof message.peer_id !== "string") return;
            if (!peers.has(message.peer_id)) return;
            addFeed("peer_left", message.peer_id);
            peers.delete(message.peer_id);
            // Packets go out while the ring still has its old layout; the
            // node fades once they arrive. State is already updated, so any
            // render in between shows the leave too.
            showTraffic({
                kind: "signal",
                type: "peer_left",
                from: "server",
                to: [myId, ...peers.keys()],
            });
            setTimeout(render, LEG_MS);
            return;
        }
        case "peer_updated": {
            if (typeof message.peer_id !== "string") return;
            const name = nameOrNull(message.name);
            if (message.peer_id === myId) {
                myName = name;
            } else if (peers.has(message.peer_id)) {
                peers.get(message.peer_id).name = name;
            } else {
                return;
            }
            render();
            addFeed("peer_updated", message.peer_id);
            showTraffic({ kind: "signal", type: "peer_updated", from: "server", to: myId });
            return;
        }
        default:
            return;
    }
}

function connect() {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const socket = new WebSocket(`${scheme}://${location.host}/ws`);

    socket.addEventListener("open", () => {
        retryDelayMs = 1000;
        setStatus("connected", "Connected");
    });

    socket.addEventListener("message", (event) => {
        let message;
        try {
            message = JSON.parse(event.data);
        } catch {
            return;
        }
        if (message && typeof message === "object") handle(message);
    });

    socket.addEventListener("close", () => {
        myId = null;
        myDevice = null;
        myName = null;
        peers = new Map();
        render();
        setStatus("connecting", "Reconnecting…");

        // Back off, with some randomness so many tabs don't all reconnect at once.
        const delay = retryDelayMs * (0.5 + Math.random());
        retryDelayMs = Math.min(retryDelayMs * 2, 15000);
        setTimeout(connect, delay);
    });
}

drawBackdrop();
render();
connect();
