use anyhow::Result;
use async_trait::async_trait;
use common::{
    auth::{AuthResponseState, LocalExAuthRequest, LocalExAuthResponse},
    event::{ClientEvent, DaemonEvent},
};
use libp2p::request_response;
use tracing::info;

const RAW_APP_ID: &str = "raw";

use crate::{
    auth::AuthHandler, file::{FileTransferClientProtocol, FilesRegisterCenter}, EventEmitter, LocalExProtocolAction, LocalExSwarm, LocalExContentProvider, PeersManager
};

#[async_trait]
pub trait ClientHandler:
    LocalExSwarm
    + EventEmitter<DaemonEvent>
    + PeersManager
    + LocalExContentProvider
    + AuthHandler
    + LocalExProtocolAction
    + FilesRegisterCenter
    + FileTransferClientProtocol
{
    async fn handle_client_cutom_message(
        &mut self,
        event: request_response::Event<Vec<u8>, Vec<u8>>,
    ) -> Result<()> {
        if let request_response::Event::Message {
            peer,
            message: request_response::Message::Request { request, .. },
        } = event
        {
            info!("received custom message from {peer}");
            self.emit_event(DaemonEvent::ReceivedCustomMessage(peer, request))
                .await?;
        }

        Ok(())
    }

    async fn handle_client_event(&mut self, event: ClientEvent) -> Result<()> {
        use ClientEvent::*;
        match event {
            VerifyConfirm(peer_id, result) => {
                let mut state = AuthResponseState::Deny;
                if result {
                    self.add_peer(peer_id);
                    self.verified(&peer_id);
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
                let _ = self
                    .emit_event(DaemonEvent::LocalInfo(
                        self.hostname(),
                        *self.swarm().local_peer_id(),
                    ))
                    .await;
            }
            RequestPeerList => {
                info!("client request peer list");
                self.send_peers().await;
            }
            RegistRaw(id, data) => {
                self.regist_raw(id, data);
            }
            RegistFileId(app_id, file_id, path) => {
                self.regist_path(&app_id, &file_id, &path)?; 
            }
            UnRegistFileId(app_id, file_id) => {
                self.unregist_file(&app_id, &file_id)?;
            }
            UnRegistAppId(app_id) => {
                self.unregist_app(&app_id)?;
            }
            UnRegistRaw(id) => {
                self.unregist_raw(&id);
            }
            SendFile(peer, app_id, file_id) => {
                info!("send {app_id}:{file_id} file to {peer}");
                self.send_file(&peer, app_id, file_id).await?;
            }
            SendRaw(peer, file_id) => {
                info!("send {file_id} raw to {peer}");
                self.send_file(&peer, RAW_APP_ID.to_string(), file_id).await?;
            }
            SendCustomMessage(peer, data) => {
                self.swarm_mut()
                    .behaviour_mut()
                    .rr_client_custom
                    .send_request(&peer, data);
            }
        }

        Ok(())
    }
}
