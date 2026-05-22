// android-update.js — Android-only: check GitHub Releases for a newer APK and
// show a dismissible banner. Tapping "Скачать" opens the APK URL in the system
// browser; the user installs it manually (sideload). Desktop has the native
// Tauri updater, so this is gated to Android user-agents only.
import { invoke } from './state.js';

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

function showUpdateBanner(info) {
  if (document.getElementById('apk-update-banner')) return;
  const bar = document.createElement('div');
  bar.id = 'apk-update-banner';
  bar.className = 'apk-update-banner';

  const text = document.createElement('span');
  text.className = 'apk-update-text';
  text.textContent = `Доступна версия ${info.version}`;

  const download = document.createElement('button');
  download.className = 'apk-update-btn';
  download.textContent = 'Скачать';
  download.addEventListener('click', () => {
    invoke('open_apk_url', { url: info.apk_url })
      .catch(e => console.error('[hanni] open apk failed', e));
  });

  const dismiss = document.createElement('button');
  dismiss.className = 'apk-update-close';
  dismiss.setAttribute('aria-label', 'Позже');
  dismiss.textContent = '×';
  dismiss.addEventListener('click', () => bar.remove());

  bar.append(text, download, dismiss);
  document.body.appendChild(bar);
}
