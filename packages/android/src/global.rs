use std::sync::Arc;

use anyhow::{Result, anyhow};
use libp2p::identity::Keypair;
use protocol::event::{ClientEvent, DaemonEvent};
use tokio::sync::{mpsc, Mutex};

use crate::service::Service;

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

pub static mut SERVICE: Option<Arc<Mutex<Service>>> = None;

pub static mut SENDER: Option<ChannelSender<DaemonEvent>> = None;
pub static mut RECEIVER: Option<ChannelReceiver<DaemonEvent>> = None;

pub static mut STOP_SINGLE_SENDER: Option<ChannelSender<bool>> = None;
pub static mut STOP_SINGLE_RECIVER: Option<ChannelReceiver<bool>> = None;

pub static mut CLIENT_EVENT_SENDER: Option<ChannelSender<ClientEvent>> = None;
pub static mut CLIENT_EVENT_RECIVER: Option<ChannelReceiver<ClientEvent>> = None;

pub fn get_or_create_service(keypair: Option<Keypair>, hostname: Option<String>) -> Result<Arc<Mutex<Service>>> {
    unsafe {
        if keypair.is_none() && SERVICE.is_none() {
            return Err(anyhow!("can't get service before initializing"))
        }

        let keypair = keypair.ok_or_else(|| anyhow!("keypair is none"))?;
        SERVICE.clone().map(Ok).unwrap_or_else(|| {
            let service = Arc::new(Mutex::new(Service::new(keypair, hostname.unwrap_or_else(|| String::from("unknown")))?));
            SERVICE = Some(service.clone());
            Ok(service)
        })
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

pub fn get_or_create_stop_single() -> Result<(ChannelSender<bool>, ChannelReceiver<bool>)> {
    unsafe {
        STOP_SINGLE_SENDER
            .clone()
            .zip(STOP_SINGLE_RECIVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(1);
                let rx = Arc::new(Mutex::new(rx));
                STOP_SINGLE_SENDER = Some(tx.clone());
                STOP_SINGLE_RECIVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}

pub fn get_or_create_client_event() -> Result<(ChannelSender<ClientEvent>, ChannelReceiver<ClientEvent>)> {
    unsafe {
        CLIENT_EVENT_SENDER
            .clone()
            .zip(CLIENT_EVENT_RECIVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(16);
                let rx = Arc::new(Mutex::new(rx));
                CLIENT_EVENT_SENDER = Some(tx.clone());
                CLIENT_EVENT_RECIVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}
