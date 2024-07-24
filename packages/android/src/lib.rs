uniffi::setup_scaffolding!();

use error::FFIError;
use ffi::{FFIClientEvent, FFIDaemonEvent};
use libp2p::identity::Keypair;
use protocol::event::ClientEvent;
use global::*;

mod global;
mod service;
mod error;
mod ffi;

#[uniffi::export]
pub fn init(hostname: String, bytekey: Vec<u8>) -> Result<(), FFIError> {
    Keypair::from_protobuf_encoding(&bytekey)
        .map_err(anyhow::Error::from)
        .and_then(|keypair| get_or_create_service(Some(keypair), Some(hostname)))
        .and(get_or_create_channel())
        .and(get_or_create_stop_single())
        .and(get_or_create_client_event())
        .map(|_| ())
        .map_err(|_| FFIError::InitError)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn dispatch(event: FFIClientEvent) -> Result<(), FFIError> {
    let sender = get_or_create_client_event().map_err(|_| FFIError::Unknown)?.0.clone();
    let event = ClientEvent::try_from(event).map_err(|_| FFIError::FFIConvertError)?; 
    sender.send(event).await
        .map(|_| ())
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
        .map(FFIDaemonEvent::from)
        .map_err(|_| FFIError::FFIConvertError)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn listen() -> Result<(), FFIError> {
    let service = get_or_create_service(None, None).map_err(|_| FFIError::Unknown)?;
    let (_, stop_rx) = get_or_create_stop_single().map_err(|_| FFIError::Unknown)?;
    let client_rx = get_or_create_client_event().map_err(|_| FFIError::Unknown)?.1.clone();
    let stop_rx = stop_rx.clone();
    let mut service_guard = service.lock().await;
    service_guard.listen(stop_rx, client_rx).await.map_err(|_| FFIError::Unknown)
}

#[uniffi::export(name = "getKeyPair")]
pub fn get_keypair() -> Option<Vec<u8>> {
    let keypair = Keypair::generate_ed25519();
    keypair.to_protobuf_encoding().ok()
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn stop() -> Result<(), FFIError> {
    get_or_create_stop_single()
        .map_err(|_| FFIError::Unknown)?
        .0
        .send(true)
        .await
        .map_err(|_| FFIError::Unknown)?;

    unsafe {
        let _ = SERVICE.take();
    }

    Ok(())
}
