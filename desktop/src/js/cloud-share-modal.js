// cloud-share-modal.js — owner-sync settings, signed-in via Google.
// Replaces the legacy service-account JSON flow with Sign in with Google
// (Stage C.1). Auto-sync section unchanged.

import { invoke, listen } from './state.js';
import { escapeHtml } from './utils.js';
import { attachOwnerSync } from './cloud-owner-sync.js';

export async function openCloudShareModal() {
  let auth = { configured: false, authenticated: false };
  let auto = { enabled: false, interval_secs: 60 };
  try { auth = await invoke('google_auth_status'); } catch {}
  try { auto = await invoke('cloud_owner_get_auto'); } catch {}

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:520px;max-height:90vh;overflow-y:auto">
    <div class="modal-title">☁ Синхронизация устройств</div>
    <p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px">
      Войди один раз через Google — данные между Mac и Android синхронизируются автоматически через Firestore. Безопасность: per-user (видишь только свои данные).
    </p>

    <div id="cs-google" style="border:1px solid var(--border-subtle);border-radius:8px;padding:12px;margin-bottom:16px"></div>

    <div id="cs-sync-section" style="${auth.authenticated ? '' : 'opacity:0.4;pointer-events:none'}">
      <div style="font-weight:600;margin-bottom:8px">🔄 Авто-синхронизация</div>
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:8px">
        <label style="display:flex;align-items:center;gap:6px;cursor:pointer;font-size:13px">
          <input type="checkbox" id="cs-auto-enabled" ${auto.enabled ? 'checked' : ''}>
          Включена
        </label>
        <span style="color:var(--text-muted);font-size:12px">каждые</span>
        <input type="number" id="cs-auto-secs" min="30" max="600" step="10"
               value="${Number(auto.interval_secs) || 60}"
               style="width:72px;padding:4px 6px;border:1px solid var(--border-subtle);border-radius:4px;font-size:13px">
        <span style="color:var(--text-muted);font-size:12px">сек</span>
      </div>
      <div id="cs-owner-status" style="font-size:12px;color:var(--text-muted);margin-bottom:8px"></div>
      <div id="cs-auto-tick" style="font-size:11px;color:var(--text-muted);margin-bottom:8px"></div>
      <div style="display:flex;gap:8px;flex-wrap:wrap">
        <button class="btn-secondary" id="cs-owner-push">⬆ Push</button>
        <button class="btn-secondary" id="cs-owner-pull">⬇ Pull</button>
        <button class="btn-primary"   id="cs-owner-sync">🔄 Sync now</button>
      </div>
      <div id="cs-owner-msg" style="min-height:18px;font-size:13px;margin-top:8px"></div>
      <div style="display:none">
        <input id="cs-owner" readonly>
        <button id="cs-owner-copy"></button>
        <input id="cs-owner-import">
        <button id="cs-owner-import-btn"></button>
      </div>
    </div>

    <div class="modal-actions" style="margin-top:16px">
      <button class="btn-secondary" id="cs-close">Закрыть</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  let unlistenTick = null;
  let unlistenAuth = null;
  function teardown() {
    if (typeof unlistenTick === 'function') { try { unlistenTick(); } catch {} }
    if (typeof unlistenAuth === 'function') { try { unlistenAuth(); } catch {} }
    overlay.remove();
  }
  overlay.addEventListener('click', (e) => { if (e.target === overlay) teardown(); });
  overlay.querySelector('#cs-close').onclick = () => teardown();

  const googleBox  = overlay.querySelector('#cs-google');
  const syncSection = overlay.querySelector('#cs-sync-section');

  function renderGoogleBox(status) {
    if (!status?.configured) {
      googleBox.innerHTML = `<div style="color:var(--color-red);font-size:13px">
        ⚠ Не настроено. Нужны OAuth Client ID и Client Secret.
        <div style="color:var(--text-muted);font-size:11px;margin-top:4px">
          Запусти setup из Claude через Playwright или вызови <code>google_auth_set_config</code> вручную.
        </div></div>`;
      syncSection.style.opacity = '0.4';
      syncSection.style.pointerEvents = 'none';
      return;
    }
    if (status.authenticated) {
      googleBox.innerHTML = `<div style="display:flex;align-items:center;justify-content:space-between;gap:12px">
        <div>
          <div style="font-size:13px">✓ Вход выполнен</div>
          <div style="color:var(--text-muted);font-size:12px;font-family:ui-monospace,Menlo,monospace">${escapeHtml(status.email || '')}</div>
          <div style="color:var(--text-muted);font-size:11px;font-family:ui-monospace,Menlo,monospace">uid: ${escapeHtml((status.uid || '').slice(0, 16))}…</div>
        </div>
        <button class="btn-secondary" id="cs-signout">Выйти</button>
      </div>`;
      syncSection.style.opacity = '';
      syncSection.style.pointerEvents = '';
      googleBox.querySelector('#cs-signout').onclick = async () => {
        if (!confirm('Выйти из аккаунта Google? Авто-sync остановится.')) return;
        try {
          await invoke('google_auth_signout');
          renderGoogleBox(await invoke('google_auth_status'));
        } catch (e) { alert('Ошибка: ' + (e?.message || e)); }
      };
    } else {
      googleBox.innerHTML = `<div style="display:flex;align-items:center;justify-content:space-between;gap:12px">
        <div>
          <div style="font-size:13px">Не выполнен вход</div>
          <div style="color:var(--text-muted);font-size:11px">Нажми кнопку — откроется системный браузер.</div>
        </div>
        <button class="btn-primary" id="cs-signin">Войти через Google</button>
      </div>`;
      syncSection.style.opacity = '0.4';
      syncSection.style.pointerEvents = 'none';
      googleBox.querySelector('#cs-signin').onclick = async () => {
        try {
          const url = await invoke('google_auth_start_signin');
          await invoke('open_url', { url });
          googleBox.innerHTML += `<div style="color:var(--text-muted);font-size:11px;margin-top:8px">Жду подтверждения в браузере…</div>`;
        } catch (e) { alert('Ошибка: ' + (e?.message || e)); }
      };
    }
  }
  renderGoogleBox(auth);

  // Live updates when callback completes in HTTP server.
  listen('google-auth-changed', async () => {
    try { renderGoogleBox(await invoke('google_auth_status')); } catch {}
  }).then((fn) => { unlistenAuth = fn; }).catch(() => {});

  unlistenTick = attachOwnerSync(overlay, auto);
}
