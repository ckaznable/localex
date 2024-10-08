uniffi::setup_scaffolding!();

use error::FFIError;
use ffi::{FFIClientEvent, FFIDaemonEvent};
use protocol::libp2p::identity::Keypair;
use common::event::ClientEvent;
use global::*;

#[cfg(target_os = "android")]
use logger::init_logger;

mod file;
mod global;
mod service;
mod error;
mod ffi;

#[uniffi::export]
pub fn init(hostname: String, bytekey: Option<Vec<u8>>, fs_dir: String, peers: Option<Vec<u8>>) -> Result<(), FFIError> {
    #[cfg(target_os = "android")]
    init_logger();

    init_quit_channel();
    get_or_create_client_event_channel()?;
    bytekey
        .and_then(|secret| Keypair::from_protobuf_encoding(&secret).ok())
        .or_else(|| Some(Keypair::generate_ed25519()))
        .zip(get_or_create_channel().ok())
        .and_then(|(keypair, (sender, _))| get_or_create_service(Some(keypair), Some(hostname), Some(sender), Some(fs_dir), peers).ok())
        .ok_or(FFIError::InitError)
        .map(|_| ())
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn dispatch(event: FFIClientEvent) -> Result<(), FFIError> {
    let event = ClientEvent::try_from(event).map_err(|_| FFIError::FFIConvertError)?; 
    get_client_event_sender()
        .map_err(|_| FFIError::FFIChannelError)?
        .send(event.into())
        .await
        .map_err(|_| FFIError::FFIChannelError)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn recv() -> Result<FFIDaemonEvent, FFIError> {
    get_or_create_channel()
        .map_err(|_| FFIError::Unknown)?
        .1
        .clone()
        .lock()
        .await
        .recv()
        .await
        .ok_or(FFIError::Unknown)
        .map_err(|_| FFIError::FFIConvertError)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn listen() -> Result<(), FFIError> {
    get_service()?
        .lock()
        .await
        .listen()
        .await
        .map_err(|_| FFIError::Unknown)
}

#[uniffi::export(name = "getKeyPair")]
pub fn get_keypair() -> Option<Vec<u8>> {
    let keypair = Keypair::generate_ed25519();
    keypair.to_protobuf_encoding().ok()
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn stop() -> Result<(), FFIError> {
    broadcast_quit();

    unsafe {
        let _ = SERVICE.take();
    }

    Ok(())
}
