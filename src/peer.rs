use tokio::sync::mpsc;
use uuid::Uuid;

use crate::messages::SignalingMessage;

#[derive(Clone)]
pub struct Peer {
    pub id: Uuid,
    pub inbox_sender: mpsc::Sender<SignalingMessage>,
}
