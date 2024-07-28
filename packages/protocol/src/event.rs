use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::peer::DaemonPeer;

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    RequestLocalInfo,
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DaemonEvent {
    VerifyResult(PeerId, bool),
    InComingVerify(DaemonPeer),
    PeerList(Vec<DaemonPeer>),
    LocalInfo(String, PeerId),
    Unknown,
}
