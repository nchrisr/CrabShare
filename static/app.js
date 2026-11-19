"use strict";

// Live list of the other devices on this network, kept in sync with the
// server's welcome / peer_joined / peer_left messages.

const ADJECTIVES = [
    "Amber", "Brave", "Calm", "Dusty", "Eager", "Fuzzy", "Gentle", "Happy",
    "Icy", "Jolly", "Kind", "Lucky", "Mellow", "Nimble", "Olive", "Plucky",
];
const ANIMALS = [
    "Otter", "Heron", "Badger", "Falcon", "Gecko", "Lynx", "Marmot", "Newt",
    "Owl", "Panda", "Quokka", "Robin", "Seal", "Tapir", "Wombat", "Yak",
];

const statusEl = document.getElementById("status");
const myNameEl = document.getElementById("my-name");
const peersEl = document.getElementById("peers");
const emptyEl = document.getElementById("empty");

let myId = null;
let peers = new Set();
let retryDelayMs = 1000;

// A readable name derived from the peer ID, so every device shows the same
// name for the same peer without the server sending one.
function nameFor(id) {
    const hex = id.replace(/-/g, "");
    const adjective = ADJECTIVES[parseInt(hex.slice(0, 2), 16) % ADJECTIVES.length];
    const animal = ANIMALS[parseInt(hex.slice(2, 4), 16) % ANIMALS.length];
    return `${adjective} ${animal}`;
}

function setStatus(state, text) {
    statusEl.dataset.state = state;
    statusEl.textContent = text;
}

function render() {
    myNameEl.textContent = myId ? nameFor(myId) : "…";
    myNameEl.title = myId ?? "";

    peersEl.replaceChildren();
    for (const id of [...peers].sort()) {
        const item = document.createElement("li");
        const name = document.createElement("span");
        name.textContent = nameFor(id);
        const shortId = document.createElement("span");
        shortId.className = "id";
        shortId.textContent = id.slice(0, 8);
        item.append(name, shortId);
        peersEl.append(item);
    }
    emptyEl.hidden = peers.size > 0;
}

function handle(message) {
    switch (message.type) {
        case "welcome":
            myId = message.peer_id;
            peers = new Set(message.peers);
            break;
        case "peer_joined":
            peers.add(message.peer_id);
            break;
        case "peer_left":
            // Deleting an ID we never saw is a no-op, which is what we want.
            peers.delete(message.peer_id);
            break;
        default:
            return;
    }
    render();
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
        handle(message);
    });

    socket.addEventListener("close", () => {
        myId = null;
        peers.clear();
        render();
        setStatus("connecting", "Reconnecting…");

        // Back off, with some randomness so many tabs don't all reconnect at once.
        const delay = retryDelayMs * (0.5 + Math.random());
        retryDelayMs = Math.min(retryDelayMs * 2, 15000);
        setTimeout(connect, delay);
    });
}

render();
connect();
