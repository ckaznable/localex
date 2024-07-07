use anyhow::Result;
use behaviour::{LocalExBehaviour, LocalExBehaviourEvent};
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{gossipsub, mdns, SwarmBuilder};
use secret::SecretStore;

mod behaviour;
mod protocol;
mod secret;

#[tokio::main]
async fn main() -> Result<()> {
    let store = SecretStore::new().await?;

    let (signed_public_key, signed_private_key) = store.get_signed_keypair().await.expect("can't get signed keypair");
    let local_keypair = store.get_local_key().await.expect("can't get libp2p keypair");
    let local_peer_id = local_keypair.public().to_peer_id();
    let mut localex = protocol::LocalExProtocol::new(local_peer_id);

    store.save_local_key(&local_keypair).await?;
    store.save_signed_keypair(&signed_public_key, &signed_private_key).await?;

    let mut swarm = SwarmBuilder::with_existing_identity(local_keypair)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(LocalExBehaviour::new)?
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;


    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => println!("Local node is listening on {address}"),
            SwarmEvent::Behaviour(event) => match event {
                LocalExBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported { peer_id }) => {
                    localex.remove_peer(&peer_id);
                },
                LocalExBehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, addr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        swarm.behaviour_mut().rr_auth.add_address(&peer_id, addr);
                        localex.add_peer(peer_id);
                    }
                },
                LocalExBehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, addr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        swarm.behaviour_mut().rr_auth.remove_address(&peer_id, &addr);
                        localex.remove_peer(&peer_id);
                    }
                },
                _ => {}
            },
            _ => {}
        }
    }
}
