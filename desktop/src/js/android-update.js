// android-update.js — Android-only: poll GitHub Releases for a newer APK,
// show a dismissible banner. Tap "Скачать" downloads the APK in-app with
// progress, then hands it to the OS package installer (one tap → installer
// dialog, no Chrome/Files round-trip).
import { invoke, listen } from './state.js';

const IS_ANDROID = /android/i.test(navigator.userAgent);

export async function checkAndroidUpdate() {
  if (!IS_ANDROID) return;
  let info;
  try {
    info = await invoke('check_apk_update');
  } catch (e) {
    console.warn('[hanni] apk update check failed', e);
    return;
  }
  if (info?.available) showUpdateBanner(info);
}

// Silently pull a newer web-asset bundle (HTML/JS/CSS) in the background so
// JS/CSS-only changes don't need a full ~106MB APK reinstall. Downloaded +
// verified + applied to app_data_dir/web/current; the custom protocol serves
// it on the next launch. Gated by min_native_version (the bundle must be
// compatible with the installed native shell). No-op until CI ships a
// web-manifest.json in the release, and harmless if anything fails (the
// protocol falls back to the APK-embedded assets).
export async function checkWebUpdate() {
  if (!IS_ANDROID) return;
  try {
    const u = await invoke('web_ota_check');
    if (!u?.available) return;
    await invoke('web_ota_apply', { url: u.url, webVersion: u.web_version, sha256: u.sha256 });
    // Applied — the custom protocol serves the new bundle on next launch.
  } catch (e) {
    console.warn('[hanni] web ota failed', e);
  }
}

// Reaching app init means the currently-served bundle loaded fine. Confirm it so
// verify_trial_on_boot keeps it instead of reverting to embedded next launch.
// No-op when there's no trial bundle pending.
export async function confirmWebBoot() {
  if (!IS_ANDROID) return;
  try { await invoke('web_ota_boot_ok'); } catch {}
}

function showUpdateBanner(info) {
  if (document.getElementById('apk-update-banner')) return;
  const bar = document.createElement('div');
  bar.id = 'apk-update-banner';
  bar.className = 'apk-update-banner';

  const text = document.createElement('span');
  text.className = 'apk-update-text';
  text.textContent = `Доступна версия ${info.version}`;

  const progress = document.createElement('div');
  progress.className = 'apk-update-progress';
  const progressFill = document.createElement('div');
  progressFill.className = 'apk-update-progress-fill';
  progress.appendChild(progressFill);
  progress.style.display = 'none';

  const action = document.createElement('button');
  action.className = 'apk-update-btn';
  action.textContent = 'Скачать';

  const dismiss = document.createElement('button');
  dismiss.className = 'apk-update-close';
  dismiss.setAttribute('aria-label', 'Позже');
  dismiss.textContent = '×';
  dismiss.addEventListener('click', () => bar.remove());

  let unlistenProgress = null;
  action.addEventListener('click', async () => {
    action.disabled = true;
    action.textContent = '0%';
    progress.style.display = 'block';

    // Subscribe to download progress events before kicking off the download.
    unlistenProgress = await listen('apk-download-progress', (e) => {
      const { loaded, total } = e.payload || {};
      if (total > 0) {
        const pct = Math.round((loaded / total) * 100);
        progressFill.style.width = pct + '%';
        action.textContent = pct + '%';
      } else {
        // Unknown total — show indeterminate label.
        action.textContent = Math.round(loaded / 1024 / 1024) + ' МБ';
      }
    });

    try {
      // 1. Make sure "Install unknown apps" is granted. If not, deep-link the
      //    user to the OS settings; they come back and tap Скачать again.
      const canInstall = await invoke('can_install_apk').catch(() => false);
      if (!canInstall) {
        await invoke('open_install_settings');
        text.textContent = 'Разреши установку и нажми Скачать ещё раз';
        action.textContent = 'Скачать';
        action.disabled = false;
        progress.style.display = 'none';
        return;
      }
      // 2. Download APK to app cache (Rust emits progress events).
      const path = await invoke('download_apk', { url: info.apk_url, version: info.version });
      // 3. Hand the file to the OS installer.
      action.textContent = 'Установка…';
      await invoke('install_apk', { path });
      // The OS installer takes over; banner stays so user can dismiss later.
    } catch (e) {
      console.error('[hanni] apk update failed', e);
      text.textContent = 'Не удалось обновить: ' + (e?.message || e);
      action.textContent = 'Повторить';
      action.disabled = false;
      progress.style.display = 'none';
    } finally {
      if (unlistenProgress) { unlistenProgress(); unlistenProgress = null; }
    }
  });

  bar.append(text, progress, action, dismiss);
  document.body.appendChild(bar);
}
