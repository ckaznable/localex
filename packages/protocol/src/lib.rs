use std::collections::BTreeMap;

use anyhow::Result;
use async_trait::async_trait;
use auth::AuthHandler;
use client::ClientHandler;
use common::{
    event::DaemonEvent,
    peer::{DaemonPeer, PeerVerifyState},
};
use file::FileTransferClientProtocol;
use libp2p::{
    mdns,
    PeerId, Swarm,
};
use message::GossipsubHandler;
use network::{LocalExBehaviour, LocalExBehaviourEvent};
use tokio::sync::broadcast;

pub mod auth;
pub mod client;
pub mod file;
pub mod message;

pub use libp2p;
pub use database;

pub trait LocalExSwarm {
    fn swarm(&self) -> &Swarm<LocalExBehaviour>;
    fn swarm_mut(&mut self) -> &mut Swarm<LocalExBehaviour>;
}

pub trait AbortListener {
    fn abort_rx(&self) -> broadcast::Receiver<()>;
}

#[async_trait]
pub trait EventEmitter<T> {
    async fn emit_event(&mut self, event: T) -> Result<()>;
}

pub trait PeersManager: LocalExSwarm {
    fn peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer>;
    fn get_peers(&mut self) -> Vec<DaemonPeer>;
    fn save_peers(&mut self) -> Result<()>;
    fn on_remove_peer(&mut self, _: &PeerId);
    fn on_add_peer(&mut self, _: PeerId);

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
}

#[async_trait]
pub trait LocalExProtocolAction: EventEmitter<DaemonEvent> + PeersManager {
    async fn send_peers(&mut self) {
        let list = self.get_peers();
        let _ = self.emit_event(DaemonEvent::PeerList(list)).await;
    }
}

pub trait LocalExContentProvider {
    fn hostname(&self) -> String;
}

#[async_trait]
pub trait LocalExProtocol:
    Send
    + LocalExSwarm
    + FileTransferClientProtocol
    + EventEmitter<DaemonEvent>
    + GossipsubHandler
    + PeersManager
    + LocalExProtocolAction
    + LocalExContentProvider
    + AuthHandler
    + ClientHandler
{
    fn prepare(&mut self) -> Result<()> {
        self.listen_on()?;
        self.subscribe_topics()?;
        Ok(())
    }

    fn listen_on(&mut self) -> Result<()> {
        let swarm = self.swarm_mut();
        swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        Ok(())
    }

    async fn handle_event(&mut self, event: LocalExBehaviourEvent) -> Result<()> {
        use LocalExBehaviourEvent::*;
        match event {
            RrAuth(event) => self.handle_auth(event).await,
            Gossipsub(event) => self.handle_gossipsub(event).await,
            Mdns(event) => self.handle_mdns(event).await,
            RrFile(event) => self.handle_file_event(event).await,
            RrClientCustom(event) => self.handle_client_cutom_message(event).await,
        }
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
}
