use anyhow::Result;
use jni::{objects::JClass, sys::jobject, JNIEnv};

pub unsafe fn create_jverify(env: &mut JNIEnv, class: &JClass) -> Result<jobject> {
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
pub unsafe fn create_jblocked(env: &mut JNIEnv, class: &JClass) -> Result<jobject> {
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
pub unsafe fn create_jwaiting_for_verification(
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
