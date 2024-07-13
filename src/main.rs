#![allow(unused_must_use)]

use std::time::Duration;

use anyhow::Result;
use behaviour::{AuthResponseState, LocalExAuthResponse, LocalExBehaviour, LocalExBehaviourEvent};
use clap::Parser;
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::swarm::SwarmEvent;
use libp2p::{gossipsub, mdns, request_response, SwarmBuilder};
use protocol::{ClientEvent, DaemonEvent};
use secret::SecretStore;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, error};
use tui::Tui;

use crate::behaviour::LocalExAuthRequest;

mod behaviour;
mod cli;
mod logger;
mod protocol;
mod secret;
mod tui;

#[tokio::main]
async fn main() -> Result<()> {
    let param = cli::Cli::parse();
    let (daemon_tx, daemon_rx) = mpsc::channel(64);
    let (client_tx, client_rx) = mpsc::channel(64);
    let (quit_tx, quit_rx) = oneshot::channel::<()>();

    let _param = param.clone();
    let handle = tokio::spawn(async move { handle_daemon(_param, quit_rx, daemon_tx, client_rx).await });

    if param.tui {
        let mut tui = Tui::new(quit_tx, client_tx, daemon_rx)?;
        tui.run().await?;
    }

    handle.await?
}

async fn handle_daemon(mut param: cli::Cli, mut quit_rx: tokio::sync::oneshot::Receiver<()>, daemon_tx: mpsc::Sender<DaemonEvent>, mut client_rx: mpsc::Receiver<ClientEvent>) -> Result<()> {
    let mut localex = protocol::LocalExProtocol::new();
    let store = SecretStore::new().await?;

    let local_keypair = if param.new_profile {
        Keypair::generate_ed25519()
    } else {
        store
            .get_local_key()
            .await
            .expect("can't get libp2p keypair")
    };

    if !param.no_save {
        store.save_local_key(&local_keypair).await?;
    }

    let log_filename = format!("{}.log", param.log_file_name.take().unwrap_or_else(|| local_keypair.public().to_peer_id().to_string()));
    logger::init_logger(log_filename, param.tui);

    let mut swarm = SwarmBuilder::with_existing_identity(local_keypair)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(LocalExBehaviour::new)?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(30)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    let local_hostname = hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::from("unknown"));

    loop {
        tokio::select! {
            _ = &mut quit_rx => break,
            client_event = client_rx.recv() => if let Some(event) = client_event {
                match event {
                    ClientEvent::VerifyConfirm(peer_id, result) => {
                        let mut state = AuthResponseState::Deny;
                        if result {
                            localex.verified(&peer_id);
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            state = AuthResponseState::Accept;
                        }

                        if let Some(channel) = localex.get_peer_mut(&peer_id).unwrap().channel.take() {
                            swarm.behaviour_mut().rr_auth.send_response(channel, LocalExAuthResponse {
                                state,
                                hostname: local_hostname.to_owned(),
                            });
                        };
                    }
                    ClientEvent::DisconnectPeer(peer_id) => {
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        localex.remove_peer(&peer_id);
                    }
                    ClientEvent::RequestVerify(peer_id) => {
                        info!("send verification request to {}", peer_id);
                        swarm.behaviour_mut().rr_auth.send_request(&peer_id, LocalExAuthRequest {
                            hostname: local_hostname.to_owned(),
                        });
                    }
                }
            },
            swarm_event = swarm.select_next_some() => if let SwarmEvent::Behaviour(event) = swarm_event {
                match event {
                    LocalExBehaviourEvent::RrAuth(request_response::Event::InboundFailure { error, .. }) => {
                        error!("inbound failure: {error}");
                    }
                    LocalExBehaviourEvent::RrAuth(request_response::Event::OutboundFailure { error, .. }) => {
                        error!("outbound failure: {error}");
                    }
                    LocalExBehaviourEvent::RrAuth(request_response::Event::Message { peer, message: request_response::Message::Request { request, channel, .. }}) => {
                        info!("{} verfication request incomming", peer);
                        localex.add_peer(peer);

                        let peer = localex.get_peer_mut(&peer).unwrap();
                        peer.set_hostname(request.hostname)
                            .set_channel(channel);

                        daemon_tx.send(DaemonEvent::InCommingVerify(peer.clone())).await;
                    }
                    LocalExBehaviourEvent::RrAuth(request_response::Event::Message { peer, message: request_response::Message::Response { response, .. }}) => {
                        let result = response.state == AuthResponseState::Accept;
                        info!("{} verify result is {}", peer, result);

                        if result {
                            localex.verified(&peer);
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                        }

                        daemon_tx.send(DaemonEvent::VerifyResult(peer, result)).await;
                    }
                    LocalExBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported { peer_id }) => {
                        localex.remove_peer(&peer_id);
                    }
                    LocalExBehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                        for (peer_id, _) in list {
                            localex.add_peer(peer_id);
                        }

                        daemon_tx.send(DaemonEvent::PeerList(localex.get_all_peers())).await;
                    }
                    LocalExBehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                        for (peer_id, _) in list {
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            localex.remove_peer(&peer_id);
                        }

                        daemon_tx.send(DaemonEvent::PeerList(localex.get_all_peers())).await;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
