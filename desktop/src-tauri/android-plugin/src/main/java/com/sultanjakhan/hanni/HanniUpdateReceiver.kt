package com.sultanjakhan.hanni

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Runs one immediate catch-up after an APK update without opening the UI. */
class HanniUpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        HanniLanSyncWorker.ensureScheduled(context)
        HanniLanSyncWorker.enqueueCatchup(context)
    }
}
