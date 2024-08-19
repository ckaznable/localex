use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::peer::DaemonPeer;

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientFileId {
    Path(String),
    Raw(Vec<u8>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    RequestLocalInfo,
    RequestPeerList,
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
    RegistFileId(String, ClientFileId),
    UnRegistFileId(String),
    SendFile(PeerId, String),
    SendCustomMessage(PeerId, Vec<u8>)
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DaemonEvent {
    VerifyResult(PeerId, bool),
    InComingVerify(DaemonPeer),
    PeerList(Vec<DaemonPeer>),
    LocalInfo(String, PeerId),
    ReceivedCustomMessage(PeerId, Vec<u8>),
    FileUpdated(String, String),
    Unknown,
}
