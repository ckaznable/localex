use anyhow::Result;
use jni::objects::{JClass, JObject, JValue};
use jni::sys::jobject;
use jni::JNIEnv;
use protocol::peer::{DaemonPeer, PeerVerifyState};

pub mod daemon_event;
pub mod client_event;
pub mod verify_state;
mod util;

pub unsafe fn create_jdaemon_peer(
    env: &mut JNIEnv,
    class: &JClass,
    dp: DaemonPeer,
) -> Result<jobject> {
    let peer_id = env.new_string(dp.peer_id.to_string())?;
    let peer_id = JObject::from_raw(peer_id.as_raw());

    let hostname = env.new_string(
        dp.hostname
            .clone()
            .unwrap_or_else(|| String::from("unkonnw")),
    )?;
    let hostname = JObject::from_raw(hostname.as_raw());

    let state = match dp.state {
        PeerVerifyState::Verified => verify_state::create_jverify(env, class),
        PeerVerifyState::Blocked => verify_state::create_jblocked(env, class),
        PeerVerifyState::WaitingVerification => {
            verify_state::create_jwaiting_for_verification(env, class)
        }
    }?;
    let state = JObject::from_raw(state);

    let class = env.find_class("io/ckaznable/localax/localex/DaemonPeer")?;
    let object = env.new_object(
        class,
        "(Ljava/lang/String;Lio/ckaznable/localax/localex/VerifyState;Ljava/lang/String;)V",
        &[
            JValue::Object(&peer_id),
            JValue::Object(&state),
            JValue::Object(&hostname),
        ],
    )?;

    Ok(object.into_raw())
}
