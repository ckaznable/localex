use std::{io, time::Duration};

use libp2p::{
    gossipsub,
    identity::Keypair,
    mdns,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    PeerId, StreamProtocol,
};
use protocol::auth::{LocalExAuthRequest, LocalExAuthResponse};

#[derive(NetworkBehaviour)]
pub struct LocalExBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub rr_auth: request_response::cbor::Behaviour<LocalExAuthRequest, LocalExAuthResponse>,
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

    fn create_auth_request_response(
    ) -> request_response::cbor::Behaviour<LocalExAuthRequest, LocalExAuthResponse> {
        let protocol = [(
            StreamProtocol::new("/localex/auth/1.0.0"),
            ProtocolSupport::Full,
        )];
        let cfg = request_response::Config::default().with_request_timeout(Duration::from_secs(30));

        request_response::cbor::Behaviour::new(protocol, cfg)
    }

    fn create_gossipsub_behavior(id_keys: Keypair) -> gossipsub::Behaviour {
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(60))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .duplicate_cache_time(Duration::from_secs(5))
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
