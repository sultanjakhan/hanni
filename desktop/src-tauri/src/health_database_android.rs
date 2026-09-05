//! JNI only. This entry point is not registered as a Tauri/WebView command.
use jni::{objects::{JClass, JString}, sys::jstring, JNIEnv};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[no_mangle]
pub extern "system" fn Java_com_sultanjakhan_hanni_HealthDatabaseNative_nativeInvoke(
    mut env: JNIEnv<'_>, _class: JClass<'_>, request: JString<'_>,
) -> jstring {
    let reply = catch_unwind(AssertUnwindSafe(|| {
        let request: String = env.get_string(&request).map_err(|_| ())?.into();
        Ok::<_, ()>(crate::health_database::reply(&request, crate::cloud_relay::open_existing))
    })).ok().and_then(Result::ok)
        .unwrap_or_else(|| r#"{"ok":false,"error":"native_db_failed"}"#.to_owned());
    if env.exception_check().unwrap_or(true) {
        crate::health_database::discard_undelivered_reply(&reply);
        return std::ptr::null_mut();
    }
    match env.new_string(&reply) {
        Ok(value) => value.into_raw(),
        Err(_) => {
            crate::health_database::discard_undelivered_reply(&reply);
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalStateException", "Native database response unavailable");
            }
            std::ptr::null_mut()
        }
    }
}
