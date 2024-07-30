use std::{
    collections::HashMap,
    path::PathBuf,
    time::Duration,
};

use anyhow::Result;
use futures::{executor::block_on, StreamExt};
use libp2p::{
    gossipsub::{self, TopicHash},
    identity::Keypair,
    mdns,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use localex_ipc::IPCServer;
use network::{LocalExBehaviour, LocalExBehaviourEvent};
use protocol::{
    auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse},
    event::{ClientEvent, DaemonEvent},
    peer::{DaemonPeer, PeerVerifyState},
};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::store::DaemonDataStore;

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipTopic {
    Hostname,
}

pub struct Daemon<'a> {
    swarm: Swarm<LocalExBehaviour>,
    server: IPCServer,
    topics: HashMap<GossipTopic, TopicHash>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: &'a str,
    ctrlc_rx: mpsc::Receiver<()>,
    store: Box<dyn DaemonDataStore>,
}

impl<'a> Daemon<'a> {
    pub fn new(local_keypair: Keypair, hostname: &'a str, sock: Option<PathBuf>, store: Box<dyn DaemonDataStore>) -> Result<Self> {
        let server = IPCServer::new(sock)?;
        let swarm = network::new_swarm(local_keypair)?;

        let (tx, rx) = mpsc::channel(1);
        let _ = ctrlc::set_handler(move || {
            block_on(async {
                tx.send(()).await
            }).expect("close application error");
        });

        Ok(Self {
            swarm,
            server,
            hostname,
            store,
            topics: HashMap::new(),
            auth_channels: HashMap::new(),
            ctrlc_rx: rx,
        })
    }

    pub fn listen_on(&mut self) -> Result<()> {
        self.swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let hostname_topic = gossipsub::IdentTopic::new("hostname-broadcaset");
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&hostname_topic)?;
        self.topics
            .insert(GossipTopic::Hostname, hostname_topic.hash());

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        self.server.prepare().await?;

        let mut hostname_broadcast_interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            let hostname_broadcast_tick = hostname_broadcast_interval.tick();

            tokio::select! {
                _ = hostname_broadcast_tick => self.broadcast_hostname(),
                    event = self.server.recv() => {
                    if let Err(e) = self.handle_client_event(event).await {
                        error!("client event error: {e:?}");
                    }
                },
                _ = self.ctrlc_rx.recv() => {
                    self.server.release();
                    return Ok(())
                },
                swarm_event = self.swarm.select_next_some() => if let SwarmEvent::Behaviour(event) = swarm_event {
                    match event {
                        LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await?,
                        LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await?,
                        LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await?,
                    }
                }
            }
        }
    }

    fn broadcast_hostname(&mut self) {
        info!("broadcast hostname to peers");
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
            self.topics.get(&GossipTopic::Hostname).unwrap().clone(),
            self.hostname,
        ) {
            error!("hostname publish error: {e:?}");
        }
    }

    async fn handle_client_event(&mut self, event: ClientEvent) -> Result<()> {
        use ClientEvent::*;
        match event {
            VerifyConfirm(peer_id, result) => {
                let mut state = AuthResponseState::Deny;
                if result {
                    self.verified(&peer_id);
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    state = AuthResponseState::Accept;
                }

                if let Some(channel) = self.auth_channels.remove(&peer_id) {
                    let _ = self.swarm.behaviour_mut().rr_auth.send_response(
                        channel,
                        LocalExAuthResponse {
                            state,
                            hostname: String::from(self.hostname),
                        },
                    );
                };
            }
            DisconnectPeer(peer_id) => {
                self.remove_peer(&peer_id);
            }
            RequestVerify(peer_id) => {
                info!("send verification request to {}", peer_id);
                self.swarm.behaviour_mut().rr_auth.send_request(
                    &peer_id,
                    LocalExAuthRequest {
                        hostname: String::from(self.hostname),
                    },
                );
            }
            RequestLocalInfo => {
                self.server
                    .broadcast(DaemonEvent::LocalInfo(
                        String::from(self.hostname),
                        *self.swarm.local_peer_id(),
                    ))
                    .await;
            }
        }

        Ok(())
    }

    async fn handle_mdns(&mut self, event: mdns::Event) -> Result<()> {
        use mdns::Event::*;
        match event {
            Discovered(list) => {
                for (peer_id, _) in list {
                    self.add_peer(peer_id);
                }

                self.broadcast_peers().await;
            }
            Expired(list) => {
                for (peer_id, _) in list {
                    self.remove_peer(&peer_id);
                }

                self.broadcast_peers().await;
            }
        }

        Ok(())
    }

    async fn handle_gossipsub(&mut self, event: gossipsub::Event) -> Result<()> {
        use gossipsub::Event::*;
        match event {
            Subscribed { topic, peer_id } => {
                if topic == *self.topics.get(&GossipTopic::Hostname).unwrap() {
                    info!("{} just subscribed hostname topic", peer_id.to_string());
                    self.broadcast_hostname();
                }
            }
            Message { message, .. } => {
                if let Some(peer) = message.source.and_then(|peer| self.store.get_peers_mut().get_mut(&peer)) {
                    let hostname =
                        String::from_utf8(message.data).unwrap_or_else(|_| String::from("unknown"));
                    info!("receive hostname broadcaset {hostname}");
                    peer.set_hostname(hostname);
                }

                self.broadcast_peers().await;
            }
            GossipsubNotSupported { peer_id } => {
                info!("{} not support gossipsub", peer_id);
                self.remove_peer(&peer_id);
                self.broadcast_peers().await;
            }
            Unsubscribed { peer_id, .. } => {
                info!("{} unsubscribe topic", peer_id);
            }
        }

        Ok(())
    }

    async fn handle_auth(
        &mut self,
        event: request_response::Event<LocalExAuthRequest, LocalExAuthResponse>,
    ) -> Result<()> {
        use request_response::Event::*;
        match event {
            InboundFailure { error, .. } => {
                error!("inbound failure: {error}");
            }
            OutboundFailure { error, .. } => {
                error!("outbound failure: {error}");
            }
            Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
            } => {
                info!(
                    "{}:{} verfication request incomming",
                    &request.hostname, peer
                );
                self.add_peer(peer);
                self.auth_channels.insert(peer, channel);

                let peer = self.store.get_peers_mut().get_mut(&peer).unwrap();
                peer.set_hostname(request.hostname);

                self.server
                    .broadcast(DaemonEvent::InComingVerify(peer.clone()))
                    .await;
                self.broadcast_peers().await;
            }
            Message {
                peer,
                message: request_response::Message::Response { response, .. },
            } => {
                let result = response.state == AuthResponseState::Accept;
                info!(
                    "{}:{} verify result is {}",
                    &response.hostname, peer, result
                );

                if result {
                    self.verified(&peer);
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer);
                }

                self.store
                    .get_peers_mut()
                    .get_mut(&peer)
                    .map(|p| p.set_hostname(response.hostname));
                let _ = self.store.save_peers();

                self.server
                    .broadcast(DaemonEvent::VerifyResult(peer, result))
                    .await;
                self.broadcast_peers().await;
            }
            _ => {}
        }

        Ok(())
    }

    fn remove_peer(&mut self, peer_id: &PeerId) {
        self.store.remove_peer(peer_id);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .remove_explicit_peer(peer_id);
    }

    fn add_peer(&mut self, peer_id: PeerId) {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer_id);

        self.store.add_peer(DaemonPeer::new(peer_id));
    }

    #[inline]
    fn verified(&mut self, peer_id: &PeerId) {
        if let Some(peer) = self.store.get_peers_mut().get_mut(peer_id) {
            peer.state = PeerVerifyState::Verified;
        }
    }

    async fn broadcast_peers(&mut self) {
        let list = self.store.get_peers().values().cloned().collect();
        self.server.broadcast(DaemonEvent::PeerList(list)).await
    }
}
