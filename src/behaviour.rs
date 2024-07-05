use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io,
    time::Duration,
};

use libp2p::{
    gossipsub, identify, identity::{Keypair, PublicKey}, kad::{self, store::MemoryStore}, mdns, swarm::NetworkBehaviour, PeerId
};

#[derive(NetworkBehaviour)]
pub struct LocalExBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
}

impl LocalExBehaviour {
    pub fn new(key: &Keypair) -> Self {
        Self {
            gossipsub: Self::create_gossipsub_behavior(key.clone()),
            mdns: Self::create_mdns_behavior(PeerId::from(key.public())),
            identify: Self::create_identify_behavior(key.public()),
            kademlia: Self::create_kademlia_behavior(PeerId::from(key.public())),
        }
    }

    fn create_kademlia_behavior(local_peer_id: PeerId) -> kad::Behaviour<MemoryStore> {
        let mut cfg = kad::Config::default();
        cfg.set_query_timeout(Duration::from_secs(10));
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, cfg);
        kademlia.set_mode(Some(kad::Mode::Server));
        kademlia
    }

    fn create_mdns_behavior(local_peer_id: PeerId) -> mdns::tokio::Behaviour {
        mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap()
    }

    fn create_identify_behavior(local_public_key: PublicKey) -> identify::Behaviour {
        let protocol_name = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        println!("{:?}", &protocol_name);
        let id_cfg = identify::Config::new(protocol_name, local_public_key);
        identify::Behaviour::new(id_cfg)
    }

    fn create_gossipsub_behavior(id_keys: Keypair) -> gossipsub::Behaviour {
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
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
