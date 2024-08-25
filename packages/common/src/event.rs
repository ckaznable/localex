use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::peer::DaemonPeer;

#[derive(Clone, Serialize, Deserialize)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    RequestLocalInfo,
    RequestPeerList,
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
    RegistFileId(String, String, String),
    RegistRaw(String, Vec<u8>),
    UnRegistRaw(String),
    UnRegistFileId(String, String),
    UnRegistAppId(String),
    SendFile(PeerId, String, String),
    SendRaw(PeerId, String),
    SendCustomMessage(PeerId, Vec<u8>)
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DaemonEvent {
    VerifyResult(PeerId, bool),
    InComingVerify(DaemonPeer),
    PeerList(Vec<DaemonPeer>),
    LocalInfo(String, PeerId),
    ReceivedCustomMessage(PeerId, Vec<u8>),
    FileUpdated(String, String, String),
    Unknown,
}
