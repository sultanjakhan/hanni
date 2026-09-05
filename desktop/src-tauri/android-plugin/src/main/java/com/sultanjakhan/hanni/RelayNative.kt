package com.sultanjakhan.hanni

import androidx.annotation.Keep

/** Loading the shared library does not call the Tauri Activity entry point. */
@Keep
internal object RelayNative {
    init { System.loadLibrary("hanni_lib") }

    @Keep
    @JvmStatic
    external fun nativeRunOnce(dbPath: String, configJson: String): String

    @Keep
    @JvmStatic
    external fun nativeProjectOnce(dbPath: String, configJson: String): String
}
