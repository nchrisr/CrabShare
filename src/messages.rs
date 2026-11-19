use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    PeerJoined {
        peer_id: Uuid,
    },
    PeerLeft {
        peer_id: Uuid,
    },
    Offer {
        from: Uuid,
        to: Uuid,
        sdp: String,
    },
    Answer {
        from: Uuid,
        to: Uuid,
        sdp: String,
    },
    IceCandidate {
        from: Uuid,
        to: Uuid,
        candidate: String,
    },
    Welcome {
        peer_id: Uuid,
        peers: Vec<Uuid>,
    },
}

impl SignalingMessage {
    /// The message's wire tag, the same string serde writes as `"type"`.
    /// Use this, never `{:?}`, when logging a message: `Debug` would print
    /// SDP and ICE payloads, which must never be logged (D9).
    pub fn type_name(&self) -> &'static str {
        // Every variant is listed with no `_` arm, so adding a variant
        // without a name here is a compile error.
        match self {
            SignalingMessage::PeerJoined { .. } => "peer_joined",
            SignalingMessage::PeerLeft { .. } => "peer_left",
            SignalingMessage::Offer { .. } => "offer",
            SignalingMessage::Answer { .. } => "answer",
            SignalingMessage::IceCandidate { .. } => "ice_candidate",
            SignalingMessage::Welcome { .. } => "welcome",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_of_each_variant() -> Vec<SignalingMessage> {
        let id = Uuid::new_v4();
        vec![
            SignalingMessage::PeerJoined { peer_id: id },
            SignalingMessage::PeerLeft { peer_id: id },
            SignalingMessage::Offer {
                from: id,
                to: id,
                sdp: String::new(),
            },
            SignalingMessage::Answer {
                from: id,
                to: id,
                sdp: String::new(),
            },
            SignalingMessage::IceCandidate {
                from: id,
                to: id,
                candidate: String::new(),
            },
            SignalingMessage::Welcome {
                peer_id: id,
                peers: Vec::new(),
            },
        ]
    }

    #[test]
    fn type_name_matches_serialized_tag() {
        for message in one_of_each_variant() {
            let json: serde_json::Value = serde_json::to_value(&message)
                .expect("SignalingMessage should always serialize to JSON");
            assert_eq!(json["type"], message.type_name());
        }
    }
}
