use std::sync::Arc;

use libp2p::identity::Keypair;
use protocol::event::DaemonEvent;
use tokio::sync::{mpsc, Mutex};

use crate::{error::FFIError, service::ServiceManager};

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

pub static mut SERVICE: Option<Arc<Mutex<ServiceManager>>> = None;

pub static mut SENDER: Option<ChannelSender<DaemonEvent>> = None;
pub static mut RECEIVER: Option<ChannelReceiver<DaemonEvent>> = None;

pub fn get_service() -> Result<Arc<Mutex<ServiceManager>>, FFIError> {
    get_or_create_service(None, None, None)
}

pub fn get_or_create_service(keypair: Option<Keypair>, hostname: Option<String>, daemon_tx: Option<mpsc::Sender<DaemonEvent>>) -> Result<Arc<Mutex<ServiceManager>>, FFIError> {
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
            ).map_err(|_| FFIError::CreateServiceError)?;

            let service = Arc::new(Mutex::new(service));
            SERVICE = Some(service.clone());
            Ok(service)
        })
    }
}

pub fn get_or_create_channel() -> Result<(ChannelSender<DaemonEvent>, ChannelReceiver<DaemonEvent>), FFIError> {
    unsafe {
        SENDER
            .clone()
            .zip(RECEIVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(32);
                let rx = Arc::new(Mutex::new(rx));
                SENDER = Some(tx.clone());
                RECEIVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}

