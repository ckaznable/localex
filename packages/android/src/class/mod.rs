use anyhow::Result;
use jni::objects::{JClass, JObject, JValue};
use jni::sys::jobject;
use jni::JNIEnv;
use protocol::peer::{DaemonPeer, PeerVerifyState};

#[allow(non_snake_case)]
pub unsafe fn create_DaemonPeer(
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
        PeerVerifyState::Verified => verify_state::create_Verify(env, class),
        PeerVerifyState::Blocked => verify_state::create_Blocked(env, class),
        PeerVerifyState::WaitingVerification => {
            verify_state::create_WaitingForVerification(env, class)
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

#[allow(non_snake_case)]
pub(crate) mod verify_state {
    use anyhow::Result;
    use jni::{objects::JClass, sys::jobject, JNIEnv};

    pub unsafe fn create_Verify(env: &mut JNIEnv, class: &JClass) -> Result<jobject> {
        Ok(env
            .get_static_field(
                class,
                "Verify",
                "Lio/ckaznable/localax/localex/VerifyState;",
            )?
            .l()?
            .into_raw())
    }

    #[no_mangle]
    pub unsafe fn create_Blocked(env: &mut JNIEnv, class: &JClass) -> Result<jobject> {
        Ok(env
            .get_static_field(
                class,
                "Blocked",
                "Lio/ckaznable/localax/localex/VerifyState;",
            )?
            .l()?
            .into_raw())
    }

    #[no_mangle]
    pub unsafe fn create_WaitingForVerification(
        env: &mut JNIEnv,
        class: &JClass,
    ) -> Result<jobject> {
        Ok(env
            .get_static_field(
                class,
                "WaitingForVerification",
                "Lio/ckaznable/localax/localex/VerifyState;",
            )?
            .l()?
            .into_raw())
    }
}

#[allow(non_snake_case)]
pub(crate) mod client_event {
    use anyhow::Result;
    use jni::objects::{JObject, JValue};
    use jni::sys::{jboolean, jobject};
    use jni::JNIEnv;

    pub unsafe fn create_RequestVerify(env: &mut JNIEnv, peer_id: String) -> Result<jobject> {
        let peer_id = env.new_string(peer_id)?;
        let peer_id = JObject::from_raw(peer_id.as_raw());

        let class = env.find_class("io/ckaznable/localax/ClientEvent$RequestVerify")?;
        let object = env.new_object(class, "(Ljava/lang/String;)V", &[JValue::Object(&peer_id)])?;

        Ok(object.into_raw())
    }

    pub unsafe fn create_DisconnectPeer(env: &mut JNIEnv, peer_id: String) -> Result<jobject> {
        let peer_id = env.new_string(peer_id)?;
        let peer_id = JObject::from_raw(peer_id.as_raw());

        let class = env.find_class("io/ckaznable/localax/ClientEvent$DisconnectPeer")?;
        let object = env.new_object(class, "(Ljava/lang/String;)V", &[JValue::Object(&peer_id)])?;

        Ok(object.into_raw())
    }

    pub unsafe fn create_VerifyConfirm(
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
}

#[allow(non_snake_case)]
pub(crate) mod daemon_event {
    use anyhow::{anyhow, Result};
    use jni::objects::{JClass, JObject, JValue};
    use jni::sys::{jboolean, jobject};
    use jni::JNIEnv;
    use protocol::event::DaemonEvent;
    use protocol::peer::DaemonPeer;

    use super::create_DaemonPeer;

    pub unsafe fn get_jdaemon_event(env: &mut JNIEnv, class: &JClass, e: DaemonEvent) -> Result<jobject> {
        match e {
            DaemonEvent::VerifyResult(p, result) => create_VerifyResult(env, p.to_string(), result),
            DaemonEvent::InComingVerify(p) => create_InComingVerify(env, class, p),
            DaemonEvent::PeerList(list) => create_PeerList(env, class, list),
            DaemonEvent::LocalInfo(_, _) => Err(anyhow!("unreachable")),
        }
    }

    pub unsafe fn create_VerifyResult(
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

    pub unsafe fn create_InComingVerify(
        env: &mut JNIEnv,
        class: &JClass,
        peer: DaemonPeer,
    ) -> Result<jobject> {
        let peer = create_DaemonPeer(env, class, peer)?;
        let peer = JObject::from_raw(peer);
        let class = env.find_class("io/ckaznable/localax/localex/DaemonEvent$InComingVerify")?;
        let object = env.new_object(
            class,
            "(Lio/ckaznable/localax/localex/DaemonPeer;)V",
            &[JValue::Object(&peer)],
        )?;

        Ok(object.into_raw())
    }

    pub unsafe fn create_PeerList(
        env: &mut JNIEnv,
        class: &JClass,
        list: Vec<DaemonPeer>,
    ) -> Result<jobject> {
        let arraylist_class = env.find_class("java/util/ArrayList")?;
        let arraylist = env.new_object(arraylist_class, "()V", &[])?;
        let arraylist_class = env.find_class("java/util/ArrayList")?;
        let add_method = env.get_method_id(arraylist_class, "add", "(Ljava/lang/Object;)Z")?;

        for dp in list {
            let peer = create_DaemonPeer(env, class, dp)?;
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
}
