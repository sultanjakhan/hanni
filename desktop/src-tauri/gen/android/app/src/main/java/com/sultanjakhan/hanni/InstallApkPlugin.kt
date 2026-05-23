package com.sultanjakhan.hanni

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class InstallApkArgs {
  lateinit var path: String
}

// Launches the OS package installer for a freshly-downloaded APK.
// The file is served via FileProvider (content:// URI) because Android 7+
// blocks file:// URIs across apps.
@TauriPlugin
class InstallApkPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun installApk(invoke: Invoke) {
    val args = invoke.parseArgs(InstallApkArgs::class.java)
    val file = File(args.path)
    if (!file.exists()) {
      invoke.reject("APK not found: ${args.path}")
      return
    }
    val uri: Uri = FileProvider.getUriForFile(
      activity, "${activity.packageName}.fileprovider", file
    )
    val intent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    try {
      activity.startActivity(intent)
      invoke.resolve(JSObject().put("launched", true))
    } catch (e: Exception) {
      invoke.reject("Failed to launch installer: ${e.message}")
    }
  }

  // True if the user has granted "Install unknown apps" for this package.
  // When false, the install intent will be silently ignored — caller should
  // open the system settings via openInstallSettings() first.
  @Command
  fun canInstall(invoke: Invoke) {
    val ok = activity.packageManager.canRequestPackageInstalls()
    invoke.resolve(JSObject().put("granted", ok))
  }

  @Command
  fun openInstallSettings(invoke: Invoke) {
    val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
      data = Uri.parse("package:${activity.packageName}")
      addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    try {
      activity.startActivity(intent)
      invoke.resolve(JSObject().put("opened", true))
    } catch (e: Exception) {
      invoke.reject("Failed to open settings: ${e.message}")
    }
  }
}
