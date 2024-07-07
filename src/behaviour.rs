use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io,
    time::Duration,
};

use libp2p::{
    gossipsub, identity::Keypair, mdns, request_response::{self, ProtocolSupport}, swarm::NetworkBehaviour, PeerId,
    StreamProtocol,
};

#[derive(serde_repr::Serialize_repr, serde_repr::Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
pub enum StreamKind {
    Auth,
    Handshake,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalExAuthRequest {
    kind: StreamKind,
    public_key: Option<Vec<u8>>,
    hostname: Option<String>,
}

#[derive(NetworkBehaviour)]
pub struct LocalExBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub rr_auth: request_response::cbor::Behaviour<LocalExAuthRequest, LocalExAuthRequest>,
}

impl LocalExBehaviour {
    pub fn new(key: &Keypair) -> Self {
        Self {
            gossipsub: Self::create_gossipsub_behavior(key.clone()),
            mdns: Self::create_mdns_behavior(PeerId::from(key.public())),
            rr_auth: Self::create_auth_request_response(),
        }
    }

    fn create_mdns_behavior(local_peer_id: PeerId) -> mdns::tokio::Behaviour {
        mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap()
    }

    fn create_auth_request_response() -> request_response::cbor::Behaviour<LocalExAuthRequest, LocalExAuthRequest> {
        let endpoint = [
            (
                StreamProtocol::new("/auth"),
                ProtocolSupport::Full,
            ),
            (
                StreamProtocol::new("/handshake"),
                ProtocolSupport::Full,
            )
        ];

        let cfg = request_response::Config::default()
            .with_request_timeout(Duration::from_secs(30)); 

        request_response::cbor::Behaviour::new(endpoint, cfg)
    }

    fn create_gossipsub_behavior(id_keys: Keypair) -> gossipsub::Behaviour {
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))
            .unwrap();

        gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(id_keys),
            gossipsub_config,
        )
        .unwrap()
    }
}
