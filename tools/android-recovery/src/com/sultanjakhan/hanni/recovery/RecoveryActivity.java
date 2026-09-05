package com.sultanjakhan.hanni.recovery;

import android.app.Activity;
import android.os.Bundle;
import android.system.Os;
import android.system.OsConstants;
import android.widget.TextView;
import java.io.*;
import java.net.*;
import java.nio.file.*;
import java.security.*;
import java.security.spec.X509EncodedKeySpec;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/** Framework-only entry point. No Tauri, SQLite, WorkManager, providers or app writes. */
public final class RecoveryActivity extends Activity {
  private static final AtomicBoolean STARTED = new AtomicBoolean();
  private volatile Socket socket;
  private TextView status;
  @Override public void onCreate(Bundle state) {
    super.onCreate(state); status = new TextView(this); status.setPadding(24, 24, 24, 24); setContentView(status);
    int port = getIntent().getIntExtra("collector_port", -1);
    if (port < 1024 || port > 65535 || !STARTED.compareAndSet(false, true)) { show("PRESERVATION_REFUSED"); return; }
    show("PRESERVATION_STARTED");
    new Thread(() -> transfer(port), "hanni-forensic-copy").start();
  }
  private void transfer(int port) {
    ScheduledExecutorService watchdog = Executors.newSingleThreadScheduledExecutor();
    socket = new Socket();
    watchdog.schedule(this::closeSocket, 180, TimeUnit.SECONDS);
    try {
      byte[] publicBytes;
      try (InputStream asset = getAssets().open("recipient.der"); ByteArrayOutputStream buffer = new ByteArrayOutputStream()) {
        byte[] part = new byte[2048]; int count; while ((count = asset.read(part)) >= 0) { if (buffer.size() + count > 8192) throw new IOException("recipient_rejected"); buffer.write(part, 0, count); } publicBytes = buffer.toByteArray();
      }
      PublicKey recipient = KeyFactory.getInstance("RSA").generatePublic(new X509EncodedKeySpec(publicBytes));
      Path root = new File(getApplicationInfo().dataDir).getCanonicalFile().toPath();
      if (Os.lstat(root.toString()).st_uid != android.os.Process.myUid() || !OsConstants.S_ISDIR(Os.lstat(root.toString()).st_mode)) throw new IOException("root_rejected");
      socket.connect(new InetSocketAddress("127.0.0.1", port), 10000); socket.setSoTimeout(30000);
      Preserver.Result result = Preserver.export(root, socket.getOutputStream(), recipient);
      socket.shutdownOutput();
      // adb reverse may close both directions on EOF. Durable acceptance is
      // verified by the Windows collector after fsync, GCM and inventory checks.
      show((result.complete ? "PRESERVATION_SENT files=" : "PRESERVATION_PARTIAL_SENT files=") + result.files);
    } catch (Throwable failure) { show("PRESERVATION_FAILED"); }
    finally { closeSocket(); watchdog.shutdownNow(); }
  }
  private void show(String value) { runOnUiThread(() -> status.setText(value)); }
  private void closeSocket() { Socket current = socket; if (current != null) try { current.close(); } catch (IOException ignored) { } }
  @Override public void onDestroy() { closeSocket(); super.onDestroy(); }
}
