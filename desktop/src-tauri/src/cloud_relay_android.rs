//! Narrow JNI adapter. All SQLite, transport and AEAD live in cloud_relay.
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[no_mangle]
pub extern "system" fn Java_com_sultanjakhan_hanni_RelayNative_nativeRunOnce(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    db_path: JString<'_>,
    config_json: JString<'_>,
) -> jstring {
    let reply = catch_unwind(AssertUnwindSafe(|| -> Result<String, ()> {
        let path: String = env.get_string(&db_path).map_err(|_| ())?.into();
        let config: String = env.get_string(&config_json).map_err(|_| ())?.into();
        if path.len() > 4096 || config.len() > 4096 {
            return Err(());
        }
        let raw = crate::cloud_relay::run_headless_once(&path, &config).map_err(|_| ())?;
        if raw.len() > 3072 {
            return Err(());
        }
        let result: serde_json::Value = serde_json::from_str(&raw).map_err(|_| ())?;
        if !result.is_object() || !result["more_pending"].is_boolean() {
            return Err(());
        }
        Ok(serde_json::json!({"ok": true, "result": result}).to_string())
    }))
    .ok()
    .and_then(Result::ok)
    .unwrap_or_else(|| r#"{"ok":false,"error":"relay_deferred"}"#.to_owned());

    // JNI errors may already have a pending VM exception. Never format it or
    // the core error, and never let Rust unwind cross the native boundary.
    if env.exception_check().unwrap_or(true) {
        return std::ptr::null_mut();
    }
    match env.new_string(reply) {
        Ok(value) => value.into_raw(),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Relay native result unavailable",
            );
            std::ptr::null_mut()
        }
    }
}


/// No HTTP: the Android raw importer and local Worker can update UI models offline.
#[no_mangle]
pub extern "system" fn Java_com_sultanjakhan_hanni_RelayNative_nativeProjectOnce(
    mut env: JNIEnv<'_>, _class: JClass<'_>, db_path: JString<'_>, config_json: JString<'_>,
) -> jstring {
    let reply = catch_unwind(AssertUnwindSafe(|| -> Result<String, ()> {
        let path: String = env.get_string(&db_path).map_err(|_| ())?.into();
        let config: String = env.get_string(&config_json).map_err(|_| ())?.into();
        if path.len() > 4096 || config.len() > 4096 { return Err(()); }
        let raw = crate::cloud_relay::run_headless_projection_once(&path, &config).map_err(|_| ())?;
        if raw.len() > 3072 { return Err(()); }
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|_| ())?;
        if !value.is_object() || !value["more_pending"].is_boolean() || !value["status"].is_string() { return Err(()); }
        Ok(serde_json::json!({"ok":true,"result":value}).to_string())
    })).ok().and_then(Result::ok).unwrap_or_else(|| r#"{"ok":false,"error":"projection_deferred"}"#.to_owned());
    if env.exception_check().unwrap_or(true) { return std::ptr::null_mut(); }
    match env.new_string(reply) {
        Ok(value) => value.into_raw(),
        Err(_) => { let _ = env.throw_new("java/lang/IllegalStateException", "Projection result unavailable"); std::ptr::null_mut() }
    }
}
