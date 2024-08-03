use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use async_trait::async_trait;
use common::{
    auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse},
    event::{ClientEvent, DaemonEvent},
    peer::{DaemonPeer, PeerVerifyState},
};
use libp2p::{
    gossipsub::{self, TopicHash},
    mdns,
    request_response::{self, ResponseChannel},
    PeerId, Swarm,
};
use network::{LocalExBehaviour, LocalExBehaviourEvent};
use tracing::{error, info};

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipTopic {
    Hostname,
}

#[allow(unused_must_use)]
#[async_trait]
pub trait LocalExProtocol: Send {
    fn hostname(&self) -> String;
    fn topics_mut(&mut self) -> &mut HashMap<GossipTopic, TopicHash>;
    fn topics(&self) -> &HashMap<GossipTopic, TopicHash>;
    fn auth_channels_mut(&mut self) -> &mut HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>;
    fn peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer>;

    fn swarm(&self) -> &Swarm<LocalExBehaviour>;
    fn swarm_mut(&mut self) -> &mut Swarm<LocalExBehaviour>;

    fn get_peers(&mut self) -> Vec<DaemonPeer>;
    fn save_peers(&mut self) -> Result<()>;

    fn on_remove_peer(&mut self, _: &PeerId);
    fn on_add_peer(&mut self, _: PeerId);

    async fn send_daemon_event(&mut self, event: DaemonEvent) -> Result<()>;

    fn broadcast_hostname(&mut self) {
        info!("broadcast hostname to peers");
        let topic = self.topics().get(&GossipTopic::Hostname).unwrap().clone();
        let hostname = self.hostname();
        if let Err(e) = self.swarm_mut().behaviour_mut().gossipsub.publish(topic, hostname) {
            error!("hostname publish error: {e:?}");
        }
    }

    async fn send_peers(&mut self) {
        let list = self.get_peers();
        self.send_daemon_event(DaemonEvent::PeerList(list)).await;
    }

    fn remove_peer(&mut self, peer_id: &PeerId) {
        self.swarm_mut()
            .behaviour_mut()
            .gossipsub
            .remove_explicit_peer(peer_id);
        self.peers_mut().remove(peer_id);
        self.on_remove_peer(peer_id);
    }

    fn verified(&mut self, peer_id: &PeerId) {
        if let Some(peer) = self.peers_mut().get_mut(peer_id) {
            peer.state = PeerVerifyState::Verified;
        }
    }

    fn add_peer(&mut self, peer_id: PeerId) {
        self.swarm_mut()
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer_id);
        self.peers_mut().insert(peer_id, DaemonPeer::new(peer_id));
        self.on_add_peer(peer_id);
    }

    fn listen_on(&mut self) -> Result<()> {
        let swarm = self.swarm_mut();
        swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let hostname_topic = gossipsub::IdentTopic::new("hostname-broadcaset");
        swarm.behaviour_mut().gossipsub.subscribe(&hostname_topic)?;
        self.topics_mut()
            .insert(GossipTopic::Hostname, hostname_topic.hash());

        Ok(())
    }

    async fn handle_event(&mut self, event: LocalExBehaviourEvent) -> Result<()> {
        match event {
            LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await,
            LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await,
            LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await,
        }
    }

    async fn handle_client_event(&mut self, event: ClientEvent) -> Result<()> {
        use ClientEvent::*;
        match event {
            VerifyConfirm(peer_id, result) => {
                let mut state = AuthResponseState::Deny;
                if result {
                    self.verified(&peer_id);
                    self.swarm_mut()
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    state = AuthResponseState::Accept;
                }

                if let Some(channel) = self.auth_channels_mut().remove(&peer_id) {
                    let hostname = self.hostname();
                    let _ = self
                        .swarm_mut()
                        .behaviour_mut()
                        .rr_auth
                        .send_response(channel, LocalExAuthResponse { state, hostname });
                };
            }
            DisconnectPeer(peer_id) => {
                self.remove_peer(&peer_id);
            }
            RequestVerify(peer_id) => {
                info!("send verification request to {}", peer_id);
                let hostname = self.hostname();
                self.swarm_mut()
                    .behaviour_mut()
                    .rr_auth
                    .send_request(&peer_id, LocalExAuthRequest { hostname });
            }
            RequestLocalInfo => {
                info!("client request local info");
                self.send_daemon_event(DaemonEvent::LocalInfo(
                    self.hostname(),
                    *self.swarm().local_peer_id(),
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

                self.send_peers().await;
            }
            Expired(list) => {
                for (peer_id, _) in list {
                    self.remove_peer(&peer_id);
                }

                self.send_peers().await;
            }
        }

        Ok(())
    }

    async fn handle_gossipsub(&mut self, event: gossipsub::Event) -> Result<()> {
        use gossipsub::Event::*;
        match event {
            Subscribed { topic, peer_id } => {
                if topic == *self.topics_mut().get(&GossipTopic::Hostname).unwrap() {
                    info!("{} just subscribed hostname topic", peer_id.to_string());
                    self.broadcast_hostname();
                }
            }
            Message { message, .. } => {
                if let Some(peer) = message
                    .source
                    .and_then(|peer| self.peers_mut().get_mut(&peer))
                {
                    let hostname =
                        String::from_utf8(message.data).unwrap_or_else(|_| String::from("unknown"));
                    info!("receive hostname broadcaset {hostname}");
                    peer.set_hostname(hostname);
                }

                self.send_peers().await;
            }
            GossipsubNotSupported { peer_id } => {
                info!("{} not support gossipsub", peer_id);
                self.remove_peer(&peer_id);
                self.send_peers().await;
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
                self.auth_channels_mut().insert(peer, channel);

                let peer = {
                    let peer = self.peers_mut().get_mut(&peer).unwrap();
                    peer.set_hostname(request.hostname);
                    peer.clone()
                };

                self.send_daemon_event(DaemonEvent::InComingVerify(peer)).await;
                self.send_peers().await;
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
                    self.swarm_mut()
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer);
                }

                self.peers_mut()
                    .get_mut(&peer)
                    .map(|p| p.set_hostname(response.hostname));
                let _ = self.save_peers();

                self.send_daemon_event(DaemonEvent::VerifyResult(peer, result)).await;
                self.send_peers().await;
            }
            _ => {}
        }

        Ok(())
    }
}
