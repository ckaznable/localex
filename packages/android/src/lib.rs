use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use global::*;

mod class;
mod global;
mod service;

#[allow(non_snake_case)]
#[allow(clippy::missing_safety_doc)]
pub mod android {
    extern crate jni;
    use self::class::daemon_event::get_jdaemon_event;

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
        mut env: JNIEnv,
        class: JClass,
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

        res.and_then(|e| get_jdaemon_event(&mut env, &class, e).map_err(|_| jni::errors::Error::JavaException))
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
        let service = get_or_create_service(None).map_err(|_| jni::errors::Error::JavaException)?;
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
