use std::sync::Arc;

use anyhow::{Result, anyhow};
use libp2p::identity::Keypair;
use protocol::event::DaemonEvent;
use tokio::{runtime::Runtime, sync::{mpsc, Mutex}, task::JoinHandle};

use crate::service::Service;

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

pub static mut RUNTIME: Option<Arc<std::sync::Mutex<Runtime>>> = None;
pub static mut SERVICE: Option<Arc<Mutex<Service>>> = None;

pub static mut SENDER: Option<ChannelSender<DaemonEvent>> = None;
pub static mut RECEIVER: Option<ChannelReceiver<DaemonEvent>> = None;

pub static mut STOP_SINGLE_SENDER: Option<ChannelSender<bool>> = None;
pub static mut STOP_SINGLE_RECVER: Option<ChannelReceiver<bool>> = None;

pub static mut LISTENER_HANDLE: Option<Arc<JoinHandle<Result<()>>>> = None;

pub fn get_or_create_runtime() -> Result<Arc<std::sync::Mutex<Runtime>>> {
    unsafe {
        RUNTIME.clone().map(Ok).unwrap_or_else(|| {
            let runtime = Arc::new(std::sync::Mutex::new(Runtime::new()?));
            RUNTIME = Some(runtime.clone());
            Ok(runtime)
        })
    }
}

pub fn get_or_create_service(keypair: Option<Keypair>) -> Result<Arc<Mutex<Service>>> {
    unsafe {
        if keypair.is_none() && SERVICE.is_none() {
            return Err(anyhow!("can't get service before initializing"))
        }

        let keypair = keypair.ok_or_else(|| anyhow!("keypair is none"))?;
        SERVICE.clone().map(Ok).unwrap_or_else(|| {
            let service = Arc::new(Mutex::new(Service::new(keypair)?));
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
            .zip(STOP_SINGLE_RECVER.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                let (tx, rx) = mpsc::channel(1);
                let rx = Arc::new(Mutex::new(rx));
                STOP_SINGLE_SENDER = Some(tx.clone());
                STOP_SINGLE_RECVER = Some(rx.clone());

                Ok((tx, rx))
            })
    }
}
