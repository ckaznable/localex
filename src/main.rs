use anyhow::Result;
use behaviour::{AuthResponseState, LocalExAuthResponse, LocalExBehaviour, LocalExBehaviourEvent};
use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{gossipsub, mdns, request_response, SwarmBuilder};
use secret::SecretStore;

mod behaviour;
mod protocol;
mod secret;

#[tokio::main]
async fn main() -> Result<()> {
    let mut localex = protocol::LocalExProtocol::new();
    let store = SecretStore::new().await?;

    let local_keypair = store.get_local_key().await.expect("can't get libp2p keypair");
    store.save_local_key(&local_keypair).await?;

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
                LocalExBehaviourEvent::RrAuth(request_response::Event::Message { peer, message: request_response::Message::Request { request, channel, .. }}) => {
                    let mut state = AuthResponseState::Deny;
                    if localex.handle_auth(peer).await {
                        localex.verified(&peer, request.hostname);
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                        state = AuthResponseState::Accept;
                    }

                    let _ = swarm.behaviour_mut().rr_auth.send_response(channel, LocalExAuthResponse {
                        state,
                        hostname: hostname::get().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|_| String::from("unknown")),
                    });
                }
                LocalExBehaviourEvent::RrAuth(request_response::Event::Message { peer, message: request_response::Message::Response { response, .. }}) => {
                    if response.state == AuthResponseState::Accept {
                        localex.verified(&peer, response.hostname);
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                    }
                }
                LocalExBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported { peer_id }) => {
                    localex.remove_peer(&peer_id);
                }
                LocalExBehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, _) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        localex.add_peer(peer_id);
                    }
                }
                LocalExBehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, _) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        localex.remove_peer(&peer_id);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}
