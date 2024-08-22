use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use common::{auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse}, event::DaemonEvent};
use libp2p::{request_response::{self, ResponseChannel}, PeerId};
use tracing::{error, info};

use crate::{EventEmitter, LocalExProtocolAction, LocalExSwarm, PeersManager};

#[async_trait]
pub trait AuthHandler: LocalExSwarm + PeersManager + EventEmitter<DaemonEvent> + LocalExProtocolAction {
    fn auth_channels_mut(&mut self) -> &mut HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>;

    async fn handle_auth(
        &mut self,
        event: request_response::Event<LocalExAuthRequest, LocalExAuthResponse>,
    ) -> Result<()> {
        use request_response::Event::*;
        match event {
            InboundFailure { error, .. } => {
                error!("rr_auth inbound failure: {error}");
            }
            OutboundFailure { error, .. } => {
                error!("rr_auth outbound failure: {error}");
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

                let _ = self.emit_event(DaemonEvent::InComingVerify(peer)).await;
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

                let _ = self
                    .emit_event(DaemonEvent::VerifyResult(peer, result))
                    .await;
                self.send_peers().await;
            }
            _ => {}
        }

        Ok(())
    }
}
