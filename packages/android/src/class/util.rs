use jni::{
    objects::{JByteArray, JClass, JObject, JString},
    JNIEnv,
};

pub unsafe fn get_class_name(env: &JNIEnv, class: &JClass) -> Result<String, jni::errors::Error> {
    let class_object = env
        .call_method(class, "getClass", "()Ljava/lang/Class;", &[])?
        .l()?;
    let name = env
        .call_method(class_object, "getName", "()Ljava/lang/String;", &[])?
        .l()?;
    let jstr = JString::from_raw(name.into_raw());
    Ok(env.get_string(&jstr)?.into())
}

pub fn get_string_field(
    env: &JNIEnv,
    obj: JObject,
    field_name: &str,
) -> Result<String, jni::errors::Error> {
    let field_value: JString = env
        .get_field(obj, field_name, "Ljava/lang/String;")?
        .l()?
        .into();
    Ok(env.get_string(&field_value)?.into())
}

pub fn get_boolean_field(
    env: &JNIEnv,
    obj: JObject,
    field_name: &str,
) -> Result<bool, jni::errors::Error> {
    Ok(env.get_field(obj, field_name, "Z")?.z()?)
}

pub unsafe fn get_byte_array_field<'a>(env: &JNIEnv<'a>, obj: JObject<'a>, field_name: &str) -> Result<Vec<u8>, jni::errors::Error> {
    let byte_array: JByteArray = env.get_field(obj, field_name, "[B")?.l()?.into();
    byte_array_to_vec_u8(env, byte_array)
}

pub fn vec_u8_to_byte_array<'a>(env: &JNIEnv, list: Vec<u8>) -> Result<JByteArray<'a>, jni::errors::Error> {
    let byte_array = env.new_byte_array(list.len() as i32)?;
    env.set_byte_array_region(byte_array, 0, &[0i8; 16])?;
    Ok(byte_array)
}

pub unsafe fn byte_array_to_vec_u8<'a>(env: &JNIEnv<'a>, array: JByteArray<'a>) -> Result<Vec<u8>, jni::errors::Error> {
    let length = env.get_array_length(&array)?;
    let mut buffer = vec![0; length as usize];
    env.get_byte_array_region(array, 0, &mut buffer)?;
    Ok(std::mem::transmute(buffer))
}
