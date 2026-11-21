use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use uuid::Uuid;

use crate::messages::SignalingMessage;

/// Every connected peer, grouped by subnet key.
pub type PeerMap = Arc<Mutex<HashMap<String, HashMap<Uuid, Peer>>>>;

#[derive(Clone)]
pub struct Peer {
    pub id: Uuid,
    pub inbox_sender: mpsc::Sender<SignalingMessage>,
}

/// Removes a peer, and its subnet if now empty; returns the peers left on that subnet.
pub fn remove_peer(
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
pub fn send_to_each(recipients: Vec<Peer>, message: SignalingMessage) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

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
