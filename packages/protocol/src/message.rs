use std::{
    collections::{HashMap, HashSet},
    fmt::Display, sync::Arc, time::Duration,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bimap::BiHashMap;
use database::entity::regist_file;
use libp2p::{
    gossipsub::{self, TopicHash},
    PeerId,
};
use tokio::{sync::{mpsc::Sender, RwLock}, task::JoinHandle, time::sleep};
use tracing::{error, info};

use crate::{
    file::{FileTransferClientProtocol, RegistFileDatabase}, LocalExContentProvider, LocalExProtocolAction, LocalExSwarm,
    PeersManager,
};

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

#[derive(Clone, Copy)]
struct PeerOffer {
    peer_id: PeerId,
    version: usize,
}

pub struct SyncRequestItem {
    pub peer_id: PeerId,
    pub app_id: String,
    pub file_id: String,
}

pub struct SyncOfferCollector {
    count: usize,
    except_count: usize,
    tx: Sender<Vec<SyncRequestItem>>,
    request_map: HashMap<(String, String), PeerOffer>,
    received_peers: HashSet<PeerId>,
}

impl SyncOfferCollector {
    fn new(except_count: usize, tx: Sender<Vec<SyncRequestItem>>) -> Self {
        Self {
            tx,
            except_count,
            count: 0,
            request_map: HashMap::new(),
            received_peers: HashSet::new(),
        }
    }

    fn insert(&mut self, app_id: String, file_id: String, offer: PeerOffer) {
        if self.received_peers.contains(&offer.peer_id) {
            return;
        }

        self.count += 1;
        self.received_peers.insert(offer.peer_id);
        let key = (app_id, file_id);
        match self.request_map.get(&key) {
            Some(inner_offer) if offer.version < inner_offer.version => {}
            _ => {
                self.request_map.insert(key, offer);
            }
        }
    }

    fn is_received(&self) -> bool {
        self.count == self.except_count
    }

    async fn consume(self) {
        let data = self
            .request_map
            .into_iter()
            .map(|((app_id, file_id), offer)| SyncRequestItem {
                app_id,
                file_id,
                peer_id: offer.peer_id,
            })
            .collect();
        let _ = self.tx.send(data).await;
    }
}

pub trait GossipTopicManager {
    fn topics_mut(&mut self) -> &mut BiHashMap<TopicHash, GossipTopic>;
    fn topics(&self) -> &BiHashMap<TopicHash, GossipTopic>;
}

#[async_trait]
pub trait GossipsubHandler:
    LocalExSwarm
    + GossipTopicManager
    + PeersManager
    + LocalExProtocolAction
    + LocalExContentProvider
    + RegistFileDatabase
    + LocalExSwarm
    + PeersManager
    + FileTransferClientProtocol
{
    fn sync_offer_collector(&self) -> Arc<RwLock<Option<SyncOfferCollector>>>;
    fn sync_offer_sender(&self) -> Sender<Vec<SyncRequestItem>>;
    fn sync_offer_timer_handler(&mut self) -> &mut Option<JoinHandle<()>>;

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

    async fn broadcast_sync_offer(&mut self) {
        if let Err(e) = self.db().get_all_regist_files().await.and_then(|data| {
            let mut writer_buf = vec![];
            ciborium::ser::into_writer(&data, &mut writer_buf)?;
            self.broadcast(GossipTopic::SyncOffer, writer_buf)
        }) {
            error!("async offer publish error: {e:?}");
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

    async fn handle_sync_offers(&mut self, offers: Vec<SyncRequestItem>) {
        for offer in offers {
            if let Err(err) = self.send_file(&offer.peer_id, offer.app_id, offer.file_id).await {
                error!("request file error: {err:?}");
            }
        }
    }

    async fn handle_gossipsub(&mut self, event: gossipsub::Event) -> Result<()> {
        use gossipsub::Event::*;
        match event {
            Subscribed { topic, peer_id } => {
                if let Some(GossipTopic::Hostname) = self.topics().get_by_left(&topic) {
                    info!("{} just subscribed hostname topic", peer_id);
                    self.broadcast_hostname();
                }
            }
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
                    GossipTopic::Sync => {
                        self.broadcast_sync_offer().await;
                    }
                    GossipTopic::SyncOffer => {
                        let Some(peer_id) = source else {
                            return Err(anyhow!("unknown source"));
                        };

                        let except_peers_count = self.peers_mut().len();
                        let collector = self.sync_offer_collector();
                        let collector_inner = collector.clone();
                        let mut guard = collector.write().await;

                        // create new collector if collector is not exist
                        if guard.is_none() {
                            let _ = guard.insert(SyncOfferCollector::new(except_peers_count, self.sync_offer_sender()));
                            // timeout
                            let handler = tokio::spawn(async move {
                                sleep(Duration::from_secs(5)).await;
                                let mut guard = collector_inner.write().await;
                                if let Some(collector) = guard.take() {
                                    collector.consume().await;
                                }
                            });

                            let timer_handler = self.sync_offer_timer_handler();
                            if let Some(last_handler) = timer_handler.replace(handler) {
                                last_handler.abort();
                            }
                        }

                        // collect the offer from other peers
                        if let Some(collector) = guard.as_mut() {
                            let list: Vec<regist_file::Model> = ciborium::from_reader(data.as_slice())?;
                            for model in list {
                                collector.insert(
                                    model.app_id,
                                    model.file_id,
                                    PeerOffer {
                                        peer_id,
                                        version: model.version as usize,
                                    },
                                )
                            }
                        }

                        // if collector is received all offers from other peers
                        if let Some(collector) = guard.take_if(|c| c.is_received()) {
                            collector.consume().await;
                            if let Some(h) = self.sync_offer_timer_handler().take() {
                                h.abort();
                            };
                        }
                    }
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
