use anyhow::{anyhow, Result};
use jni::objects::{JObject, JValue};
use jni::sys::{jboolean, jobject};
use jni::JNIEnv;
use libp2p::PeerId;
use protocol::event::ClientEvent;

use super::util::{get_boolean_field, get_byte_array_field, get_class_name};

pub unsafe fn create_jrequest_verify(env: &mut JNIEnv, peer_id: String) -> Result<jobject> {
    let peer_id = env.new_string(peer_id)?;
    let peer_id = JObject::from_raw(peer_id.as_raw());

    let class = env.find_class("io/ckaznable/localax/ClientEvent$RequestVerify")?;
    let object = env.new_object(class, "(Ljava/lang/String;)V", &[JValue::Object(&peer_id)])?;

    Ok(object.into_raw())
}

pub unsafe fn create_jdisconnect_peer(env: &mut JNIEnv, peer_id: String) -> Result<jobject> {
    let peer_id = env.new_string(peer_id)?;
    let peer_id = JObject::from_raw(peer_id.as_raw());

    let class = env.find_class("io/ckaznable/localax/ClientEvent$DisconnectPeer")?;
    let object = env.new_object(class, "(Ljava/lang/String;)V", &[JValue::Object(&peer_id)])?;

    Ok(object.into_raw())
}

pub unsafe fn create_jverify_confirm(
    env: &mut JNIEnv,
    peer_id: String,
    confirm: bool,
) -> Result<jobject> {
    let peer_id = env.new_string(peer_id)?;
    let peer_id = JObject::from_raw(peer_id.as_raw());

    let class = env.find_class("io/ckaznable/localax/localex/ClientEvent$VerifyConfirm")?;
    let object = env.new_object(
        class,
        "(Ljava/lang/String;Z)V",
        &[JValue::Object(&peer_id), JValue::Bool(confirm as jboolean)],
    )?;

    Ok(object.into_raw())
}

pub unsafe fn from_jobject<'a>(env: &mut JNIEnv<'a>, event: JObject<'a>) -> Result<ClientEvent> {
    let class = env.get_object_class(&event)?;
    let class_name = get_class_name(env, &class)?;

    match class_name.as_str() {
        "io/ckaznable/localax/localex/ClientEvent$RequestVerify" => {
            let data = get_byte_array_field(env, &event, "peerId")?;
            Ok(ClientEvent::RequestVerify(PeerId::from_bytes(&data)?))
        },
        "io/ckaznable/localax/localex/ClientEvent$DisconnectPeer" => {
            let data = get_byte_array_field(env, &event, "peerId")?;
            Ok(ClientEvent::DisconnectPeer(PeerId::from_bytes(&data)?))
        },
        "io/ckaznable/localax/localex/ClientEvent$VerifyConfirm" => {
            let data = get_byte_array_field(env, &event, "peerId")?;
            let peer_id = PeerId::from_bytes(&data)?;
            let confirm = get_boolean_field(env, event, "confirm")?;

            Ok(ClientEvent::VerifyConfirm(peer_id, confirm))
        },
        _ => Err(anyhow!("unreachable")),
    }
}
