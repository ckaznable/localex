use std::collections::HashMap;

use anyhow::Result;
use futures::StreamExt;
use libp2p::{gossipsub::{self, TopicHash}, identity::Keypair, mdns, request_response::{self, ResponseChannel}, swarm::SwarmEvent, PeerId, Swarm};
use network::{new_swarm, LocalExBehaviour, LocalExBehaviourEvent};
use protocol::{auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse}, event::ClientEvent};

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipTopic {
    Hostname,
}

pub struct Service {
    swarm: Swarm<LocalExBehaviour>,
    topics: HashMap<GossipTopic, TopicHash>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
}

impl Service {
    pub fn new(local_keypair: Keypair, hostname: String) -> Result<Self> {
        let swarm = new_swarm(local_keypair)?;
        Ok(Self {
            swarm,
            hostname,
            auth_channels: HashMap::new(),
            topics: HashMap::new(),
        })
    }

    pub async fn listen(&mut self) -> Result<()> {
        self.listen_on()?;
        self.run().await
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
        let insert = self.topics
            .insert(GossipTopic::Hostname, hostname_topic.hash());

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
             if let SwarmEvent::Behaviour(event) = self.swarm.select_next_some().await {
                match event {
                    LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await?,
                    LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await?,
                    LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await?,
                }
            }
        }
    }

    fn broadcast_hostname(&mut self) {
        self.swarm.behaviour_mut().gossipsub.publish(
            self.topics.get(&GossipTopic::Hostname).unwrap().clone(),
            self.hostname,
        );
    }

    async fn handle_client_event(&mut self, event: ClientEvent) -> Result<()> {
        use ClientEvent::*;
        match event {
            VerifyConfirm(peer_id, result) => {
                let state = if result { AuthResponseState::Accept } else { AuthResponseState::Deny };
                if let Some(channel) = self.auth_channels.remove(&peer_id) {
                    let _ =self.swarm.behaviour_mut().rr_auth.send_response(channel, LocalExAuthResponse {
                        state,
                        hostname: self.hostname.clone(),
                    });
                };
            }
            DisconnectPeer(peer_id) => {
                self.remove_peer(&peer_id)
            }
            RequestVerify(peer_id) => {
                self.swarm.behaviour_mut().rr_auth.send_request(&peer_id, LocalExAuthRequest {
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
                    todo!()
                }
            }
            Message { message, .. } => {
                todo!()
            }
            GossipsubNotSupported { peer_id } => {
                self.remove_peer(&peer_id);
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
                self.add_peer(peer);
                self.auth_channels.insert(peer, channel);
                todo!()
            }
            Message {
                peer,
                message: request_response::Message::Response { response, .. },
            } => {
                let result = response.state == AuthResponseState::Accept;

                if result {
                    self.swarm
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

    fn remove_peer(&mut self, peer_id: &PeerId) {
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
    }

    async fn broadcast_peers(&self) {
        todo!()
    }
}
