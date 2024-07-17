use libp2p::PeerId;

use crate::peer::DeamonPeer;

#[derive(Clone)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
}

#[derive(Clone)]
pub enum DeamonEvent {
    VerifyResult(PeerId, bool),
    InCommingVerify(DeamonPeer),
    PeerList(Vec<DeamonPeer>),
    LocalInfo(String, PeerId),
}
