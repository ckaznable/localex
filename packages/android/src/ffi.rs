use protocol::libp2p::PeerId;
use common::{event::{ClientEvent, DaemonEvent}, peer::{DaemonPeer, PeerVerifyState}};

use crate::error::FFIError;

#[derive(uniffi::Enum)]
pub enum FFIClientEvent {
    RequestVerify(Vec<u8>),
    RequestLocalInfo,
    RequestPeerList,
    DisconnectPeer(Vec<u8>),
    VerifyConfirm(Vec<u8>, bool),
}

impl From<ClientEvent> for FFIClientEvent {
    fn from(event: ClientEvent) -> Self {
        match event {
            ClientEvent::RequestVerify(p) => Self::RequestVerify(p.to_bytes()),
            ClientEvent::RequestLocalInfo => Self::RequestLocalInfo,
            ClientEvent::RequestPeerList => Self::RequestPeerList,
            ClientEvent::DisconnectPeer(p) => Self::DisconnectPeer(p.to_bytes()),
            ClientEvent::VerifyConfirm(p, result) => Self::VerifyConfirm(p.to_bytes(), result),
            ClientEvent::RegistFileId(_, _, _) => todo!(),
            ClientEvent::UnRegistFileId(_, _) => todo!(),
            ClientEvent::SendFile(_, _, _) => todo!(),
            ClientEvent::SendCustomMessage(_, _) => todo!(),
            ClientEvent::RegistRaw(_, _) => todo!(),
            ClientEvent::UnRegistAppId(_) => todo!(),
            ClientEvent::UnRegistRaw(_) => todo!(),
            ClientEvent::SendRaw(_, _) => todo!(),
        }
    }
}

impl TryFrom<FFIClientEvent> for ClientEvent {
    type Error = anyhow::Error;

    fn try_from(ffi: FFIClientEvent) -> Result<Self, Self::Error> {
        let event = match ffi {
            FFIClientEvent::RequestVerify(p) => Self::RequestVerify(PeerId::from_bytes(&p)?),
            FFIClientEvent::RequestLocalInfo => Self::RequestLocalInfo,
            FFIClientEvent::RequestPeerList => Self::RequestPeerList,
            FFIClientEvent::DisconnectPeer(p) => Self::DisconnectPeer(PeerId::from_bytes(&p)?),
            FFIClientEvent::VerifyConfirm(p, result) => Self::VerifyConfirm(PeerId::from_bytes(&p)?, result),
        };

        Ok(event)
    }
}

#[derive(uniffi::Enum)]
pub enum FFIDaemonEvent {
    VerifyResult(Vec<u8>, String, bool),
    InComingVerify(FFIDaemonPeer),
    PeerList(Vec<FFIDaemonPeer>),
    LocalInfo(String, Vec<u8>),
    Error(FFIError),
    Log(String),
    Unknown,
}

impl From<DaemonEvent> for FFIDaemonEvent  {
    fn from(event: DaemonEvent) -> Self {
        match event {
            DaemonEvent::VerifyResult(p, result) => Self::VerifyResult(p.to_bytes(), p.to_string(), result),
            DaemonEvent::InComingVerify(p) => Self::InComingVerify(p.into()),
            DaemonEvent::PeerList(peers) => Self::PeerList(peers.into_iter().map(|p| p.into()).collect()),
            DaemonEvent::LocalInfo(hostname, local_id) => Self::LocalInfo(hostname, local_id.to_bytes()),
            _ => Self::Unknown,
        }
    }
}

impl TryFrom<FFIDaemonEvent> for DaemonEvent {
    type Error = anyhow::Error;

    fn try_from(event: FFIDaemonEvent) -> Result<Self, Self::Error> {
        let event = match event {
            FFIDaemonEvent::VerifyResult(p, _, result) => Self::VerifyResult(PeerId::from_bytes(&p)?, result),
            FFIDaemonEvent::InComingVerify(p) => Self::InComingVerify(p.try_into()?),
            FFIDaemonEvent::LocalInfo(hostname, p) => Self::LocalInfo(hostname, PeerId::from_bytes(&p)?),
            FFIDaemonEvent::PeerList(peers) => {
                let list = peers
                    .into_iter()
                    .map(|p| p.try_into())
                    .filter_map(Result::ok)
                    .collect();
                Self::PeerList(list)
            },
            _ => Self::Unknown
        };

        Ok(event)
    }
}

#[derive(uniffi::Enum)]
pub enum FFIPeerVerifyState {
    Verified,
    Blocked,
    WaitingVerification,
}

impl From<PeerVerifyState> for FFIPeerVerifyState {
    fn from(value: PeerVerifyState) -> Self {
        match value {
            PeerVerifyState::Verified => Self::Verified,
            PeerVerifyState::Blocked => Self::Blocked,
            PeerVerifyState::WaitingVerification => Self::WaitingVerification,
        }
    }
}

impl From<FFIPeerVerifyState> for PeerVerifyState {
    fn from(value: FFIPeerVerifyState) -> Self {
        match value {
            FFIPeerVerifyState::Verified => Self::Verified,
            FFIPeerVerifyState::Blocked => Self::Blocked,
            FFIPeerVerifyState::WaitingVerification => Self::WaitingVerification,
        }
    }
}

#[derive(uniffi::Record)]
pub struct FFIDaemonPeer {
    pub peer_id: Vec<u8>,
    pub peer_id_str: String,
    pub state: FFIPeerVerifyState,
    #[uniffi(default = "unknown")]
    pub hostname: String,
}

impl From<DaemonPeer> for FFIDaemonPeer {
    fn from(value: DaemonPeer) -> Self {
        Self {
            peer_id: value.peer_id.to_bytes(),
            peer_id_str: value.peer_id.to_string(),
            state: value.state.into(),
            hostname: value.hostname.unwrap_or_else(|| String::from("unknown")),
        }
    }
}

impl TryFrom<FFIDaemonPeer> for DaemonPeer {
    type Error = anyhow::Error;

    fn try_from(value: FFIDaemonPeer) -> Result<Self, Self::Error> {
        let peer = Self {
            peer_id: PeerId::from_bytes(&value.peer_id)?,
            state: value.state.into(),
            hostname: Some(value.hostname),
        };

        Ok(peer)
    }
}
