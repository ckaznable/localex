use anyhow::{anyhow, Result};
use std::{sync::Arc, time::Duration};

use libp2p::{identity::Keypair, Swarm, SwarmBuilder};
use localex_daemon::behaviour::LocalExBehaviour;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, Mutex}, task::JoinHandle,
};

mod class;

type ChannelSender<T> = mpsc::Sender<T>;
type ChannelReceiver<T> = Arc<Mutex<mpsc::Receiver<T>>>;

static mut RUNTIME: Option<Arc<std::sync::Mutex<Runtime>>> = None;
static mut SERVICE: Option<Arc<Mutex<Service>>> = None;

static mut SENDER: Option<ChannelSender<Event>> = None;
static mut RECEIVER: Option<ChannelReceiver<Event>> = None;

static mut STOP_SINGLE_SENDER: Option<ChannelSender<bool>> = None;
static mut STOP_SINGLE_RECVER: Option<ChannelReceiver<bool>> = None;

static mut LISTENER_HANDLE: Option<Arc<JoinHandle<Result<()>>>> = None;

enum Event {}

struct Service {
    swarm: Swarm<LocalExBehaviour>,
}

impl Service {
    pub fn new(local_keypair: Keypair) -> Result<Self> {
        let swarm = SwarmBuilder::with_existing_identity(local_keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(LocalExBehaviour::new)?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(30)))
            .build();

        Ok(Self { swarm })
    }

    pub async fn listen(&mut self) -> Result<()> {
        todo!()
    }
}

fn get_or_create_runtime() -> Result<Arc<std::sync::Mutex<Runtime>>> {
    unsafe {
        RUNTIME.clone().map(Ok).unwrap_or_else(|| {
            let runtime = Arc::new(std::sync::Mutex::new(Runtime::new()?));
            RUNTIME = Some(runtime.clone());
            Ok(runtime)
        })
    }
}

fn get_or_create_service(keypair: Option<Keypair>) -> Result<Arc<Mutex<Service>>> {
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

fn get_or_create_channel() -> Result<(ChannelSender<Event>, ChannelReceiver<Event>)> {
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

fn get_or_create_stop_single() -> Result<(ChannelSender<bool>, ChannelReceiver<bool>)> {
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

#[allow(non_snake_case)]
#[allow(clippy::missing_safety_doc)]
pub mod android {
    extern crate jni;
    use super::*;
    use jni::objects::{JByteArray, JClass};
    use jni::sys::{jbyteArray, jobject};
    use jni::JNIEnv;
    use libp2p::identity::Keypair;

    #[no_mangle]
    pub unsafe extern "system" fn Java_io_ckaznable_localax_LocalEx_init(
        env: JNIEnv,
        _: JClass,
        key: JByteArray,
    ) -> jni::errors::Result<()> {
        let bytekey = convert_to_key(&env, key).map_err(|_| jni::errors::Error::JavaException)?;
        let keypair = Keypair::from_protobuf_encoding(&bytekey)
            .map_err(|_| jni::errors::Error::JavaException)?;

        let _ = get_or_create_runtime().map_err(|_| jni::errors::Error::JavaException)?;
        let _ = get_or_create_service(Some(keypair)).map_err(|_| jni::errors::Error::JavaException)?;
        let _ = get_or_create_channel().map_err(|_| jni::errors::Error::JavaException)?;
        let _ = get_or_create_stop_single().map_err(|_| jni::errors::Error::JavaException)?;

        Ok(())
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_io_ckaznable_localax_LocalEx_recv(
        _: JNIEnv,
        _: JClass,
    ) -> jni::errors::Result<jobject> {
        let runtime = get_or_create_runtime().map_err(|_| jni::errors::Error::JavaException)?.clone();
        let recv = get_or_create_channel().map_err(|_| jni::errors::Error::JavaException)?.1.clone();
        let res = runtime
            .lock()
            .map_err(|_| jni::errors::Error::JavaException)?
            .block_on(async move {
                recv
                    .lock()
                    .await
                    .recv()
                    .await
                    .ok_or_else(|| jni::errors::Error::JavaException)
            });

        res
            .map(|_| todo!())
            .map_err(|_| jni::errors::Error::JavaException)
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_io_ckaznable_localax_LocalEx_listen(
        env: JNIEnv,
        _: JClass,
    ) -> jni::errors::Result<()> {
        // already listen
        if LISTENER_HANDLE.is_some() {
            return Err(jni::errors::Error::JavaException);
        }

        let runtime = get_or_create_runtime().map_err(|_| jni::errors::Error::JavaException)?;
        let service = get_or_create_service(None).map_err(|_| jni::errors::Error::JavaException)?.clone();
        let (_, stop_rx) = get_or_create_stop_single().map_err(|_| jni::errors::Error::JavaException)?;
        let stop_rx = stop_rx.clone();

        let handle: JoinHandle<Result<()>> = runtime
            .lock()
            .map_err(|_| jni::errors::Error::JavaException)?
            .spawn(async move {
                loop {
                    tokio::select! {
                        mut s = service.lock() => s.listen().await?,
                        mut rx = stop_rx.lock() => if rx.recv().await.is_some() {
                            break
                        },
                    }
                }

                Ok(())
            });

        LISTENER_HANDLE = Some(Arc::new(handle));

        Ok(())
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_io_ckaznable_localax_LocalEx_getKeyPair(
        env: JNIEnv,
        _: JClass,
    ) -> jbyteArray {
        let keypair = Keypair::generate_ed25519();
        let bytekey = keypair.to_protobuf_encoding().unwrap_or_else(|_| vec![]);

        let byte_arr = env.byte_array_from_slice(&bytekey);
        match byte_arr {
            Ok(arr) => arr.into_raw(),
            _ => create_empty_jbyte_array(&env),
        }
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_io_ckaznable_localax_LocalEx_stop(
        _: JNIEnv,
        _: JClass,
    ) -> jni::errors::Result<()> {
        let runtime = get_or_create_runtime().map_err(|_| jni::errors::Error::JavaException)?;

        let (sender, _) = get_or_create_stop_single().map_err(|_| jni::errors::Error::JavaException)?;
        runtime
            .lock()
            .map_err(|_| jni::errors::Error::JavaException)?
            .block_on(async move {
                sender
                    .send(true)
                    .await
                    .map(|_| ())
                    .map_err(|_| jni::errors::Error::JavaException)
            })?;

        let _ = SERVICE.take();
        let _ = LISTENER_HANDLE.take();
        Ok(())
    }

    fn create_empty_jbyte_array(env: &JNIEnv) -> jbyteArray {
        match env.new_byte_array(0) {
            Ok(empty_array) => empty_array.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    fn convert_to_key(env: &JNIEnv, key: JByteArray) -> Result<Vec<u8>> {
        env.convert_byte_array(key).map_err(anyhow::Error::from)
    }
}
