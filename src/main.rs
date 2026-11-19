mod messages;
mod peer;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Mutex, mpsc};
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::messages::SignalingMessage;
use crate::peer::Peer;

type PeerMap = Arc<Mutex<HashMap<String, HashMap<Uuid, Peer>>>>;

const HOST: &str = "0.0.0.0";
const PORT: u16 = 3000;
const DEFAULT_MAX_MESSAGE_COUNT: usize = 32;
const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

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
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket: WebSocket| async move {
        // socket is the live WebSocket connection, handle it here.
        let Some(subnet) = subnet_key(addr) else {
            tracing::warn!("Rejected non-IPV4 connection from {}", addr);
            return;
        };

        let peer_id = Uuid::new_v4();
        tracing::info!("ID: {} assigned to new peer", peer_id);
        tracing::info!("Peer {} connected from subnet {}", peer_id, subnet);

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
                        Some(Err(_)) => break,
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

fn remove_peer(
    peers: &mut HashMap<String, HashMap<Uuid, Peer>>,
    subnet: &str,
    peer_id: Uuid,
) -> Vec<Peer> {
    let Some(subnet_map) = peers.get_mut(subnet) else {
        return Vec::new();
    };

    let Some(_) = subnet_map.remove(&peer_id) else {
        return Vec::new();
    };

    if subnet_map.is_empty() {
        peers.remove(subnet);
        return Vec::new();
    }

    subnet_map.values().cloned().collect()
}

/// Sends `message` to each recipient without waiting; returns how many got it.
fn send_to_each(recipients: Vec<Peer>, message: SignalingMessage) -> usize {
    let mut sent = 0;

    for recipient in recipients {
        match recipient.inbox_sender.try_send(message.clone()) {
            Ok(()) => sent += 1,
            Err(TrySendError::Full(_)) => {
                tracing::warn!(
                    "inbox of peer {} is full, dropped a {} message",
                    recipient.id,
                    message.type_name()
                );
            }
            Err(TrySendError::Closed(_)) => {
                tracing::debug!(
                    "peer {} already disconnected, skipped a {} message",
                    recipient.id,
                    message.type_name()
                );
            }
        }
    }

    sent
}

fn subnet_key(addr: SocketAddr) -> Option<String> {
    match addr.ip() {
        IpAddr::V4(ip) => {
            let octets = ip.octets(); // get an array of the parts of the IP address.
            format!("{}_{}_{}", octets[0], octets[1], octets[2]).into()
            // build the key from the first 3 octets
        }
        IpAddr::V6(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subnet_key_ipv4() {
        let _addr: SocketAddr = "198.168.1.5:3000".parse().unwrap();
        assert_eq!(subnet_key(_addr), Some("198_168_1".to_string()));
    }

    #[test]
    fn test_subnet_key_ipv6() {
        // [::1] is the ipv6 equivalent of localhost
        let _addr: SocketAddr = "[::1]:3000".parse().unwrap();
        assert_eq!(subnet_key(_addr), None);
    }

    // Test helpers. Creating an mpsc channel needs no tokio runtime, so these
    // tests are plain #[test]s.

    /// A peer plus the receiving end of its inbox, so a test can check what
    /// arrived. `capacity` is the inbox size.
    fn test_peer(capacity: usize) -> (Peer, mpsc::Receiver<SignalingMessage>) {
        let (inbox_sender, inbox) = mpsc::channel::<SignalingMessage>(capacity);
        let peer = Peer {
            id: Uuid::new_v4(),
            inbox_sender: inbox_sender,
        };
        (peer, inbox)
    }

    /// Builds a peer map from (subnet, peers) pairs.
    fn test_map(subnets: Vec<(&str, Vec<&Peer>)>) -> HashMap<String, HashMap<Uuid, Peer>> {
        let mut map = HashMap::new();
        for (subnet, peers) in subnets {
            let mut subnet_map = HashMap::new();
            for peer in peers {
                subnet_map.insert(peer.id, peer.clone());
            }
            map.insert(subnet.to_string(), subnet_map);
        }
        map
    }

    fn ids(peers: &[Peer]) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = peers.iter().map(|peer| peer.id).collect();
        ids.sort();
        ids
    }

    // remove_peer

    #[test]
    fn remove_peer_returns_the_others_on_its_subnet() {
        let (a, _a_inbox) = test_peer(1);
        let (b, _b_inbox) = test_peer(1);
        let (c, _c_inbox) = test_peer(1);
        let mut map = test_map(vec![("192_168_1", vec![&a, &b, &c])]);

        let remaining = remove_peer(&mut map, "192_168_1", a.id);

        let mut expected = vec![b.id, c.id];
        expected.sort();
        assert_eq!(ids(&remaining), expected);
        assert!(!map["192_168_1"].contains_key(&a.id));
        assert_eq!(map["192_168_1"].len(), 2);
    }

    #[test]
    fn remove_peer_drops_the_subnet_when_the_last_peer_leaves() {
        let (a, _a_inbox) = test_peer(1);
        let mut map = test_map(vec![("192_168_1", vec![&a])]);

        let remaining = remove_peer(&mut map, "192_168_1", a.id);

        assert!(remaining.is_empty());
        assert!(!map.contains_key("192_168_1"));
    }

    #[test]
    fn remove_peer_leaves_other_subnets_alone() {
        let (a, _a_inbox) = test_peer(1);
        let (other, _other_inbox) = test_peer(1);
        let mut map = test_map(vec![("192_168_1", vec![&a]), ("10_0_0", vec![&other])]);

        let remaining = remove_peer(&mut map, "192_168_1", a.id);

        assert!(remaining.is_empty());
        assert_eq!(map["10_0_0"].len(), 1);
        assert!(map["10_0_0"].contains_key(&other.id));
    }

    #[test]
    fn remove_peer_with_an_unknown_id_changes_nothing() {
        let (a, _a_inbox) = test_peer(1);
        let mut map = test_map(vec![("192_168_1", vec![&a])]);

        let remaining = remove_peer(&mut map, "192_168_1", Uuid::new_v4());
        assert!(remaining.is_empty());
        assert_eq!(map["192_168_1"].len(), 1);

        let remaining = remove_peer(&mut map, "no_such_subnet", a.id);
        assert!(remaining.is_empty());
        assert_eq!(map["192_168_1"].len(), 1);
    }

    // send_to_each

    #[test]
    fn send_to_each_delivers_to_everyone_with_room() {
        let (a, mut a_inbox) = test_peer(4);
        let (b, mut b_inbox) = test_peer(4);
        let left_id = Uuid::new_v4();

        let sent = send_to_each(vec![a, b], SignalingMessage::PeerLeft { peer_id: left_id });

        assert_eq!(sent, 2);
        for inbox in [&mut a_inbox, &mut b_inbox] {
            match inbox.try_recv() {
                Ok(SignalingMessage::PeerLeft { peer_id }) => assert_eq!(peer_id, left_id),
                other => panic!("expected peer_left, got {:?}", other.map(|m| m.type_name())),
            }
        }
    }

    #[test]
    fn send_to_each_skips_a_full_inbox_without_waiting() {
        let (full, mut full_inbox) = test_peer(1);
        let (ok, mut ok_inbox) = test_peer(1);
        let first_id = Uuid::new_v4();
        full.inbox_sender
            .try_send(SignalingMessage::PeerJoined { peer_id: first_id })
            .expect("a new inbox of capacity 1 should have room for one message");

        // If send_to_each waited on the full inbox, this test would hang here.
        let sent = send_to_each(
            vec![full, ok],
            SignalingMessage::PeerLeft {
                peer_id: Uuid::new_v4(),
            },
        );

        assert_eq!(sent, 1);
        match full_inbox.try_recv() {
            Ok(SignalingMessage::PeerJoined { peer_id }) => assert_eq!(peer_id, first_id),
            other => panic!(
                "expected the original peer_joined, got {:?}",
                other.map(|m| m.type_name())
            ),
        }
        assert!(
            full_inbox.try_recv().is_err(),
            "the full inbox should hold nothing else"
        );
        assert!(ok_inbox.try_recv().is_ok());
    }

    #[test]
    fn send_to_each_skips_a_peer_that_already_disconnected() {
        let (gone, gone_inbox) = test_peer(1);
        let (ok, _ok_inbox) = test_peer(1);
        drop(gone_inbox);

        let sent = send_to_each(
            vec![gone, ok],
            SignalingMessage::PeerLeft {
                peer_id: Uuid::new_v4(),
            },
        );

        assert_eq!(sent, 1);
    }
}
