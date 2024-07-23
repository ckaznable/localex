use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use libp2p::{gossipsub::{self, TopicHash}, identity::Keypair, mdns, request_response::{self, ResponseChannel}, swarm::SwarmEvent, PeerId, Swarm};
use network::{new_swarm, LocalExBehaviour, LocalExBehaviourEvent};
use protocol::{auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse}, event::ClientEvent};
use tokio::sync::{mpsc, Mutex, MutexGuard};

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipTopic {
    Hostname,
}

pub struct Service {
    swarm: Arc<Mutex<Swarm<LocalExBehaviour>>>,
    topics: HashMap<GossipTopic, TopicHash>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
}

impl Service {
    pub fn new(local_keypair: Keypair, hostname: String) -> Result<Self> {
        let swarm = Arc::new(Mutex::new(new_swarm(local_keypair)?));
        Ok(Self {
            swarm,
            hostname,
            auth_channels: HashMap::new(),
            topics: HashMap::new(),
        })
    }

    pub async fn listen(&mut self, quit_rx: Arc<Mutex<mpsc::Receiver<bool>>>, client_rx: Arc<Mutex<mpsc::Receiver<ClientEvent>>>) -> Result<()> {
        self.listen_on().await?;
        self.run(quit_rx, client_rx).await;
        Ok(())
    }

    pub async fn listen_on(&mut self) -> Result<()> {
        let mut swarm = self.swarm.lock().await;
        swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let hostname_topic = gossipsub::IdentTopic::new("hostname-broadcaset");
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&hostname_topic)?;
        let _ = self.topics
            .insert(GossipTopic::Hostname, hostname_topic.hash());

        Ok(())
    }

    pub async fn run(&mut self, quit_rx: Arc<Mutex<mpsc::Receiver<bool>>>, client_rx: Arc<Mutex<mpsc::Receiver<ClientEvent>>>) {
        let swarm = self.swarm.clone();
        let mut quit_rx_guard = quit_rx.lock().await;
        let mut client_rx_guard = client_rx.lock().await;

        loop {
            tokio::select! {
                _ = async {
                    let mut swarm = swarm.lock().await;
                    if let SwarmEvent::Behaviour(event) = swarm.select_next_some().await {
                        let _ = match event {
                            LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await,
                            LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await,
                            LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await,
                        };
                    }
                } => {},
                event = client_rx_guard.recv() => if let Some(event) = event {
                    let _ = self.handle_client_event(event).await;
                },
                rx = quit_rx_guard.recv()  => if rx.is_some() {
                    break
                },
            }
        }
    }

    async fn broadcast_hostname(&mut self) {
        let mut swarm = self.swarm.lock().await;
        let _ = swarm.behaviour_mut().gossipsub.publish(
            self.topics.get(&GossipTopic::Hostname).unwrap().clone(),
            self.hostname.clone(),
        );
    }

    async fn handle_client_event(&mut self, event: ClientEvent) -> Result<()> {
        use ClientEvent::*;
        match event {
            VerifyConfirm(peer_id, result) => {
                let mut swarm = self.swarm.lock().await;
                let state = if result { AuthResponseState::Accept } else { AuthResponseState::Deny };
                if let Some(channel) = self.auth_channels.remove(&peer_id) {
                    let _ = swarm.behaviour_mut().rr_auth.send_response(channel, LocalExAuthResponse {
                        state,
                        hostname: self.hostname.clone(),
                    });
                };
            }
            DisconnectPeer(peer_id) => {
                self.remove_peer(&peer_id).await;
            }
            RequestVerify(peer_id) => {
                let mut swarm = self.swarm.lock().await;
                swarm.behaviour_mut().rr_auth.send_request(&peer_id, LocalExAuthRequest {
                    hostname: self.hostname.clone(),
                });
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_mdns(&mut self, event: mdns::Event) -> Result<()> {
        use mdns::Event::*;
        match event {
            Discovered(list) => {
                let swarm = self.swarm.clone();
                let mut swarm = swarm.lock().await;
                for (peer_id, _) in list {
                    self.add_peer_with_swarm_guard(&mut swarm, peer_id).await;
                }

                self.broadcast_peers().await;
            }
            Expired(list) => {
                let swarm = self.swarm.clone();
                let mut swarm = swarm.lock().await;
                for (peer_id, _) in list {
                    self.remove_peer_with_swarm_guard(&mut swarm, &peer_id).await;
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
                    todo!()
                }
            }
            Message { message, .. } => {
                todo!()
            }
            GossipsubNotSupported { peer_id } => {
                self.remove_peer(&peer_id).await;
                todo!()
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_auth(
        &mut self,
        event: request_response::Event<LocalExAuthRequest, LocalExAuthResponse>,
    ) -> Result<()> {
        use request_response::Event::*;
        match event {
            Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
            } => {
                self.add_peer(peer).await;
                self.auth_channels.insert(peer, channel);
                todo!()
            }
            Message {
                peer,
                message: request_response::Message::Response { response, .. },
            } => {
                let result = response.state == AuthResponseState::Accept;

                if result {
                    let mut swarm = self.swarm.lock().await;
                    swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer);
                }

                todo!()
            }
            _ => {}
        }

        Ok(())
    }

    async fn remove_peer(&mut self, peer_id: &PeerId) {
        let swarm = self.swarm.clone();
        let mut swarm = swarm.lock().await;
        self.remove_peer_with_swarm_guard(&mut swarm, peer_id).await;
    }

    async fn remove_peer_with_swarm_guard<'a>(&mut self, swarm: &mut MutexGuard<'a, Swarm<LocalExBehaviour>>, peer_id: &PeerId) {
        swarm
            .behaviour_mut()
            .gossipsub
            .remove_explicit_peer(peer_id);
    }

    async fn add_peer(&mut self, peer_id: PeerId) {
        let swarm = self.swarm.clone();
        let mut swarm = swarm.lock().await;
        self.add_peer_with_swarm_guard(&mut swarm, peer_id).await;
    }

    async fn add_peer_with_swarm_guard<'a>(&mut self, swarm: &mut MutexGuard<'a, Swarm<LocalExBehaviour>>, peer_id: PeerId) {
        swarm
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer_id);
    }

    async fn broadcast_peers(&self) {
        todo!()
    }
}
