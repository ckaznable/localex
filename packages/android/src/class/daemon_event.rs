use anyhow::{anyhow, Result};
use jni::objects::{JClass, JObject, JValue};
use jni::sys::{jboolean, jobject};
use jni::JNIEnv;
use protocol::event::DaemonEvent;
use protocol::peer::DaemonPeer;

use super::create_jdaemon_peer;

pub unsafe fn get_jdaemon_event(env: &mut JNIEnv, class: &JClass, e: DaemonEvent) -> Result<jobject> {
    match e {
        DaemonEvent::VerifyResult(p, result) => create_jverify_result(env, p.to_string(), result),
        DaemonEvent::InComingVerify(p) => create_jincoming_verify(env, class, p),
        DaemonEvent::PeerList(list) => create_jpeer_list(env, class, list),
        DaemonEvent::LocalInfo(_, _) => Err(anyhow!("unreachable")),
    }
}

pub unsafe fn create_jverify_result(
    env: &mut JNIEnv,
    peer_id: String,
    result: bool,
) -> Result<jobject> {
    let peer_id = env.new_string(peer_id)?;
    let peer_id = JObject::from_raw(peer_id.as_raw());

    let class = env
        .find_class("io/ckaznable/localax/localex/DaemonEvent$VerifyResult")
        .unwrap();
    let object = env.new_object(
        class,
        "(Ljava/lang/String;Z)V",
        &[JValue::Object(&peer_id), JValue::Bool(result as jboolean)],
    )?;

    Ok(object.into_raw())
}

pub unsafe fn create_jincoming_verify(
    env: &mut JNIEnv,
    class: &JClass,
    peer: DaemonPeer,
) -> Result<jobject> {
    let peer = create_jdaemon_peer(env, class, peer)?;
    let peer = JObject::from_raw(peer);
    let class = env.find_class("io/ckaznable/localax/localex/DaemonEvent$InComingVerify")?;
    let object = env.new_object(
        class,
        "(Lio/ckaznable/localax/localex/DaemonPeer;)V",
        &[JValue::Object(&peer)],
    )?;

    Ok(object.into_raw())
}

pub unsafe fn create_jpeer_list(
    env: &mut JNIEnv,
    class: &JClass,
    list: Vec<DaemonPeer>,
) -> Result<jobject> {
    let arraylist_class = env.find_class("java/util/ArrayList")?;
    let arraylist = env.new_object(arraylist_class, "()V", &[])?;
    let arraylist_class = env.find_class("java/util/ArrayList")?;
    let add_method = env.get_method_id(arraylist_class, "add", "(Ljava/lang/Object;)Z")?;

    for dp in list {
        let peer = create_jdaemon_peer(env, class, dp)?;
        let peer = JObject::from_raw(peer);
        env.call_method_unchecked(
            &arraylist,
            add_method,
            jni::signature::ReturnType::Primitive(jni::signature::Primitive::Boolean),
            &[JValue::Object(&peer).as_jni()],
        )?;
    }

    let class = env.find_class("io/ckaznable/localax/localex/DaemonEvent$PeerList")?;
    let object = env.new_object(
        class,
        "(Ljava/util/ArrayList;)V",
        &[JValue::Object(&arraylist)],
    )?;

    Ok(object.into_raw())
}
