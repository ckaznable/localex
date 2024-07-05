use anyhow::Result;
use behaviour::LocalExBehaviour;
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{identity, SwarmBuilder};
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

    store.save_local_key(&local_key).await?;

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
            SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {address:?}"),
            SwarmEvent::Behaviour(event) => println!("{event:?}"),
            _ => {}
        }
    }
}
