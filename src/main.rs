use anyhow::Result;
use behaviour::{LocalExBehaviour, LocalExBehaviourEvent};
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{identity, mdns, SwarmBuilder};
use secret::SecretStore;

mod behaviour;
mod secret;

#[tokio::main]
async fn main() -> Result<()> {
    let store = SecretStore::new().await?;

    let local_key = store.get_local_key().await.unwrap_or_else(|| {
        println!("local key not existing, generated new one");
        identity::Keypair::generate_ed25519()
    });

    let password: Vec<u8> = store.get_password().await.unwrap_or_else(|| {
        rpassword::prompt_password("Please enter verification password: ").unwrap().as_bytes().to_vec()
    });

    store.save_local_key(&local_key).await?;
    store.save_password(&password).await?;

    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
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
                LocalExBehaviourEvent::Gossipsub(_) => todo!(),
                LocalExBehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, _) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                LocalExBehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, _) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
            },
            _ => {}
        }
    }
}
