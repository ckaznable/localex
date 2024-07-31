uniffi::setup_scaffolding!();

use error::FFIError;
use ffi::{FFIClientEvent, FFIDaemonEvent};
use futures::executor::block_on;
use libp2p::identity::Keypair;
use common::event::ClientEvent;
use global::*;

mod global;
mod service;
mod error;
mod ffi;

#[uniffi::export]
pub fn init(hostname: String, bytekey: Option<Vec<u8>>) -> Result<(), FFIError> {
    bytekey
        .and_then(|secret| Keypair::from_protobuf_encoding(&secret).ok())
        .or_else(|| Some(Keypair::generate_ed25519()))
        .zip(get_or_create_channel().ok())
        .and_then(|(keypair, (sender, _))| get_or_create_service(Some(keypair), Some(hostname), Some(sender)).ok())
        .ok_or(FFIError::InitError)
        .map(|_| ())
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn dispatch(event: FFIClientEvent) -> Result<(), FFIError> {
    let event = ClientEvent::try_from(event).map_err(|_| FFIError::FFIConvertError)?; 
    get_service()
        .map_err(|_| FFIError::FFIChannelError)?
        .lock()
        .await
        .dispatch(event)
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
    block_on(async {
        get_service()?
            .lock()
            .await
            .listen()
            .await
            .map_err(|_| FFIError::Unknown)
    })
}

#[uniffi::export(name = "getKeyPair")]
pub fn get_keypair() -> Option<Vec<u8>> {
    let keypair = Keypair::generate_ed25519();
    keypair.to_protobuf_encoding().ok()
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn stop() -> Result<(), FFIError> {
    get_service()
        .map_err(|_| FFIError::Unknown)?
        .lock()
        .await
        .quit()
        .await;

    unsafe {
        let _ = SERVICE.take();
    }

    Ok(())
}
