use jni::{
    objects::{JByteArray, JClass, JObject, JString},
    JNIEnv,
};

pub unsafe fn get_class_name(
    env: &mut JNIEnv,
    class: &JClass,
) -> Result<String, jni::errors::Error> {
    let class_object = env
        .call_method(class, "getClass", "()Ljava/lang/Class;", &[])?
        .l()?;
    let name = env
        .call_method(class_object, "getName", "()Ljava/lang/String;", &[])?
        .l()?;
    let jstr = JString::from_raw(name.into_raw());
    let name = env.get_string(&jstr)?.into();
    Ok(name)
}

pub fn get_string_field(
    env: &mut JNIEnv,
    obj: JObject,
    field_name: &str,
) -> Result<String, jni::errors::Error> {
    let field_value: JString = env
        .get_field(obj, field_name, "Ljava/lang/String;")?
        .l()?
        .into();
    let value = env.get_string(&field_value)?.into();
    Ok(value)
}

pub fn get_boolean_field(
    env: &mut JNIEnv,
    obj: JObject,
    field_name: &str,
) -> Result<bool, jni::errors::Error> {
    env.get_field(obj, field_name, "Z")?.z()
}

pub unsafe fn get_byte_array_field<'a>(
    env: &mut JNIEnv<'a>,
    obj: &JObject<'a>,
    field_name: &str,
) -> Result<Vec<u8>, jni::errors::Error> {
    let byte_array: JByteArray = env.get_field(obj, field_name, "[B")?.l()?.into();
    env.convert_byte_array(byte_array)
}
