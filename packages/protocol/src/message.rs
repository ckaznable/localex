use std::fmt::Display;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bimap::BiHashMap;
use libp2p::gossipsub::{self, TopicHash};
use tracing::{error, info};

use crate::{LocalExProtocolAction, LocalExSwarm, LocalexContentProvider, PeersManager};

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GossipTopic {
    Hostname,
    Sync,
    SyncOffer,
}

impl Display for GossipTopic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match *self {
            GossipTopic::Hostname => "hostname",
            GossipTopic::Sync => "sync",
            GossipTopic::SyncOffer => "sync_offer",
        };

        write!(f, "{s}")
    }
}

impl From<GossipTopic> for String {
    fn from(val: GossipTopic) -> Self {
        val.to_string()
    }
}

pub trait GossipTopicManager {
    fn topics_mut(&mut self) -> &mut BiHashMap<TopicHash, GossipTopic>;
    fn topics(&self) -> &BiHashMap<TopicHash, GossipTopic>;
}

#[async_trait]
pub trait GossipsubHandler:
    LocalExSwarm + GossipTopicManager + PeersManager + LocalExProtocolAction + LocalexContentProvider
{
    fn broadcast<T: Into<Vec<u8>>>(&mut self, topic: GossipTopic, data: T) -> Result<()> {
        let topic = {
            let Some(topic) = self.topics().get_by_right(&topic) else {
                return Err(anyhow!("topic is not exist"));
            };

            (*topic).clone()
        };

        self.swarm_mut()
            .behaviour_mut()
            .gossipsub
            .publish(topic, data.into())
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }

    fn broadcast_hostname(&mut self) {
        info!("broadcast hostname to peers");
        let hostname = self.hostname();
        if let Err(e) = self.broadcast(GossipTopic::Hostname, hostname) {
            error!("hostname publish error: {e:?}");
        }
    }

    fn subscribe_topics(&mut self) -> Result<()> {
        use GossipTopic::*;
        self.subscribe(Hostname)?;
        self.subscribe(Sync)?;
        self.subscribe(SyncOffer)?;
        Ok(())
    }

    fn subscribe(&mut self, topic: GossipTopic) -> Result<()> {
        let _topic = gossipsub::IdentTopic::new(topic);
        self.swarm_mut()
            .behaviour_mut()
            .gossipsub
            .subscribe(&_topic)?;
        self.topics_mut().insert(_topic.hash(), topic);

        Ok(())
    }

    async fn handle_gossipsub(&mut self, event: gossipsub::Event) -> Result<()> {
        use gossipsub::Event::*;
        match event {
            Subscribed { topic, peer_id } => match self.topics().get_by_left(&topic) {
                Some(GossipTopic::Hostname) => {
                    info!("{} just subscribed hostname topic", peer_id);
                    self.broadcast_hostname();
                }
                Some(GossipTopic::Sync) => {}
                Some(GossipTopic::SyncOffer) => {}
                _ => {}
            },
            Message { message, .. } => {
                let gossipsub::Message {
                    source,
                    data,
                    topic,
                    ..
                } = message;
                let topic = {
                    let Some(topic) = self.topics().get_by_left(&topic) else {
                        return Ok(());
                    };
                    *topic
                };

                match topic {
                    GossipTopic::Hostname => {
                        if let Some(peer) = source.and_then(|peer| self.peers_mut().get_mut(&peer))
                        {
                            let hostname =
                                String::from_utf8(data).unwrap_or_else(|_| String::from("unknown"));
                            info!("receive hostname broadcaset {hostname}");
                            peer.set_hostname(hostname);
                            self.send_peers().await;
                        };
                    }
                    GossipTopic::Sync => {}
                    GossipTopic::SyncOffer => {}
                }
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
}
