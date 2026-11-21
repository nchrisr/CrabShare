mod device;
mod messages;
mod peer;
mod subnet;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::RawQuery;
use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::http::header::USER_AGENT;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::device::connecting_device_kind;
use crate::messages::SignalingMessage;
use crate::peer::{Peer, PeerMap, remove_peer, send_to_each};
use crate::subnet::subnet_key;

const HOST: &str = "0.0.0.0";
const PORT: u16 = 3000;
const DEFAULT_MAX_MESSAGE_COUNT: usize = 32;
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest WebSocket message or frame a client may send. Signaling needs a few KB.
const MAX_INCOMING_MESSAGE_BYTES: usize = 16 * 1024;

#[tokio::main]
async fn main() {
    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    let router = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(peers);

    let address = format!("{}:{}", HOST, PORT);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let listener = TcpListener::bind(&address).await.unwrap();
    tracing::info!("Server running on {}", &address);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn ws_handler(
    State(peers): State<PeerMap>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Still plain HTTP here, before the 101 response: read the request and set limits now.
    // A header is bytes, not text; a value that isn't visible ASCII counts as missing.
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let device = connecting_device_kind(query.as_deref(), user_agent);

    let ws = ws
        .max_message_size(MAX_INCOMING_MESSAGE_BYTES)
        .max_frame_size(MAX_INCOMING_MESSAGE_BYTES);

    ws.on_upgrade(move |mut socket: WebSocket| async move {
        // socket is the live WebSocket connection, handle it here.
        let Some(subnet) = subnet_key(addr) else {
            tracing::warn!("Rejected non-IPv4 connection from {}", addr);
            return;
        };

        let peer_id = Uuid::new_v4();
        tracing::info!("ID: {} assigned to new peer", peer_id);
        tracing::info!(
            "Peer {} connected from subnet {} on {:?}",
            peer_id,
            subnet,
            device
        );

        let (inbox_sender, mut inbox) =
            mpsc::channel::<SignalingMessage>(DEFAULT_MAX_MESSAGE_COUNT);

        let other_peers = {
            let mut map = peers.lock().await;
            if !map.contains_key(&subnet) {
                map.insert(subnet.clone(), HashMap::new());
            }

            let other_peers: Vec<Peer> = map
                .get(&subnet)
                .expect("subnet map should exist because it was just created")
                .values()
                .cloned()
                .collect();

            // Queue the welcome before this peer is visible to anyone else, so it is
            // always the first message the browser receives. try_send doesn't wait,
            // which keeps the no-await-under-the-lock rule.
            let existing_peers: Vec<Uuid> = other_peers.iter().map(|other| other.id).collect();
            let welcome = SignalingMessage::Welcome {
                peer_id: peer_id,
                peers: existing_peers,
            };
            inbox_sender
                .try_send(welcome)
                .expect("new inbox should have room because nothing has been sent to it yet");

            let peer = Peer {
                id: peer_id,
                inbox_sender: inbox_sender,
            };
            map.get_mut(&subnet)
                .expect("subnet map should exist because it was just created")
                .insert(peer_id, peer);

            for key in map.keys() {
                tracing::info!(
                    "{} Active peers on subnet {}",
                    map.get(key).map(|m| m.len()).unwrap_or(0),
                    key
                );
            }

            tracing::info!("Total active subnets: {}", map.len());
            other_peers
        };

        send_to_each(
            other_peers,
            SignalingMessage::PeerJoined { peer_id: peer_id },
        );

        loop {
            tokio::select! {
                from_browser = socket.recv() => {
                    match from_browser {
                        None => break,
                        Some(Err(error)) => {
                            if is_message_too_large(error) {
                                tracing::info!("Peer {} sent a message too large, closing", peer_id);
                            }
                            break;
                        }
                        Some(Ok(_)) => {},
                    }
                }
                from_inbox = inbox.recv() => {
                    match from_inbox {
                        None => break,
                        Some(letter) => {
                            let json_msg = serde_json::to_string(&letter)
                                .expect("SignalingMessage should always serialize to JSON");
                            let message = Utf8Bytes::from(json_msg);
                            let write = socket.send(Message::Text(message));

                            // After a timeout the socket may hold half a frame, so never
                            // write to it again: leave the loop and let it drop.
                            match tokio::time::timeout(SOCKET_WRITE_TIMEOUT, write).await {
                                Ok(Ok(())) => {}
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    tracing::info!("Write to peer {} timed out", peer_id);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Close the inbox first, so anyone still holding our Sender fails at once
        // instead of filling a queue nobody reads.
        drop(inbox);

        let remaining_peers = {
            let mut map = peers.lock().await;
            remove_peer(&mut map, &subnet, peer_id)
        };

        let told = send_to_each(
            remaining_peers,
            SignalingMessage::PeerLeft { peer_id: peer_id },
        );
        tracing::info!("Peer {} disconnected, {} peers told", peer_id, told);
    })
}

/// Whether a receive error means the client went over `MAX_INCOMING_MESSAGE_BYTES`.
fn is_message_too_large(error: axum::Error) -> bool {
    // axum wraps tungstenite's error, so unwrap it and check for a capacity error.
    let inner = error.into_inner();
    matches!(
        inner.downcast_ref::<tungstenite::Error>(),
        Some(tungstenite::Error::Capacity(_))
    )
}
