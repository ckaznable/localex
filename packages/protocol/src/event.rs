use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::peer::DeamonPeer;

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    RequestLocalInfo,
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DeamonEvent {
    VerifyResult(PeerId, bool),
    InCommingVerify(DeamonPeer),
    PeerList(Vec<DeamonPeer>),
    LocalInfo(String, PeerId),
}
