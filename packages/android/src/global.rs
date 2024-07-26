use std::sync::Arc;

use anyhow::{Result, anyhow};
use libp2p::identity::Keypair;
use protocol::event::DaemonEvent;
use tokio::sync::{mpsc, Mutex};

use crate::service::ServiceManager;

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

pub static mut SERVICE: Option<Arc<Mutex<ServiceManager>>> = None;

pub static mut SENDER: Option<ChannelSender<DaemonEvent>> = None;
pub static mut RECEIVER: Option<ChannelReceiver<DaemonEvent>> = None;

pub fn get_service() -> Result<Arc<Mutex<ServiceManager>>> {
    get_or_create_service(None, None)
}

pub fn get_or_create_service(keypair: Option<Keypair>, hostname: Option<String>) -> Result<Arc<Mutex<ServiceManager>>> {
    unsafe {
        if keypair.is_none() && SERVICE.is_none() {
            return Err(anyhow!("can't get service before initializing"))
        }

        let keypair = keypair.ok_or_else(|| anyhow!("keypair is none"))?;
        SERVICE.clone().map(Ok).unwrap_or_else(|| {
            let service = Arc::new(Mutex::new(ServiceManager::new(keypair, hostname.unwrap_or_else(|| String::from("unknown")))?));
            SERVICE = Some(service.clone());
            Ok(service)
        })
    }
}

pub fn get_daemon_sender() -> Option<ChannelSender<DaemonEvent>> {
    unsafe {
        SENDER.clone()
    }
}

pub fn get_or_create_channel() -> Result<(ChannelSender<DaemonEvent>, ChannelReceiver<DaemonEvent>)> {
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

