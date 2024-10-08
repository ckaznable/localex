use std::sync::Arc;

use protocol::libp2p::identity::Keypair;
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::{error::FFIError, ffi::{FFIClientEvent, FFIDaemonEvent}, service::ServiceManager};

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

pub static mut SERVICE: Option<Arc<Mutex<ServiceManager>>> = None;

pub static mut SENDER: Option<ChannelSender<FFIDaemonEvent>> = None;
pub static mut RECEIVER: Option<ChannelReceiver<FFIDaemonEvent>> = None;

pub static mut CLIENT_EVENT_SENDER: Option<ChannelSender<FFIClientEvent>> = None;
pub static mut CLIENT_EVENT_RECEIVER: Option<ChannelReceiver<FFIClientEvent>> = None;

pub static mut QUIT_SENDER: Option<broadcast::Sender<()>> = None;
pub static mut QUIT_RECEIVER: Option<broadcast::Receiver<()>> = None;

pub fn get_service() -> Result<Arc<Mutex<ServiceManager>>, FFIError> {
    get_or_create_service(None, None, None, None, None)
}

pub fn get_or_create_service(
    keypair: Option<Keypair>,
    hostname: Option<String>,
    daemon_tx: Option<mpsc::Sender<FFIDaemonEvent>>,
    fs_dir: Option<String>,
    peers: Option<Vec<u8>>,
) -> Result<Arc<Mutex<ServiceManager>>, FFIError> {
    unsafe {
        if keypair.is_none() && SERVICE.is_none() && daemon_tx.is_none() {
            return Err(FFIError::AccessServiceBeforeInitError)
        }

        SERVICE.clone().map(Ok).unwrap_or_else(|| {
            let keypair = keypair.ok_or(FFIError::Unknown)?;
            let service = ServiceManager::new(
                keypair,
                hostname.unwrap_or_else(|| String::from("unknown")),
                daemon_tx.unwrap(),
                fs_dir.unwrap().into(),
                peers,
            ).map_err(|_| FFIError::CreateServiceError)?;

            let service = Arc::new(Mutex::new(service));
            SERVICE = Some(service.clone());
            Ok(service)
        })
    }
}

pub fn get_or_create_channel() -> Result<(ChannelSender<FFIDaemonEvent>, ChannelReceiver<FFIDaemonEvent>), FFIError> {
    unsafe {
        SENDER
            .clone()
            .zip(RECEIVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(4);
                let rx = Arc::new(Mutex::new(rx));
                SENDER = Some(tx.clone());
                RECEIVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}

pub fn get_client_event_sender() -> Result<ChannelSender<FFIClientEvent>, FFIError> {
    unsafe {
        CLIENT_EVENT_SENDER.clone().ok_or(FFIError::FFIClientEventHandleError)
    }
}

pub fn get_client_event_receiver() -> Result<ChannelReceiver<FFIClientEvent>, FFIError> {
    unsafe {
        CLIENT_EVENT_RECEIVER.clone().ok_or(FFIError::FFIClientEventHandleError)
    }
}

pub fn get_or_create_client_event_channel() -> Result<(ChannelSender<FFIClientEvent>, ChannelReceiver<FFIClientEvent>), FFIError> {
    unsafe {
        CLIENT_EVENT_SENDER
            .clone()
            .zip(CLIENT_EVENT_RECEIVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(4);
                let rx = Arc::new(Mutex::new(rx));
                CLIENT_EVENT_SENDER = Some(tx.clone());
                CLIENT_EVENT_RECEIVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}

pub fn init_quit_channel() {
    unsafe {
        let (tx, rx) = broadcast::channel(1);
        QUIT_SENDER = Some(tx);
        QUIT_RECEIVER = Some(rx);
    }
}

pub fn broadcast_quit() {
    unsafe {
        QUIT_SENDER.as_ref().map(|tx| tx.send(()).ok());
    }
}

pub fn get_quit_rx() -> Result<broadcast::Receiver<()>, FFIError> {
    unsafe {
        if let Some(recv) = QUIT_RECEIVER.as_ref() {
            Ok(recv.resubscribe())
        } else {
            Err(FFIError::InitError)
        }
    }
}
