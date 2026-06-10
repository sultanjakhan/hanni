// ── js/tabs-settings-sync.js — Sync settings section (extracted from tabs.js) ──

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

// ── Sync Settings Section ──

export async function renderSyncSection() {
  let status = { enabled: false, last_sync: null, pending_changes: 0, site_id: '', device_name: '' };
  try { status = await invoke('get_sync_status'); } catch(_) {}
  return `
    <div class="settings-section">
      <div class="settings-section-title">Синхронизация</div>
      <div class="settings-row"><span class="settings-label">Включить синхронизацию</span>
        <span class="settings-value"><label class="toggle"><input type="checkbox" id="sync-enabled" ${status.enabled ? 'checked' : ''}><span class="toggle-track"></span></label></span></div>
      <div class="settings-row"><span class="settings-label">Relay URL</span>
        <span class="settings-value"><input class="form-input" id="sync-relay-url" type="text" placeholder="https://hanni-sync.workers.dev" style="width:260px;"></span></div>
      <div class="settings-row"><span class="settings-label">Device Token</span>
        <span class="settings-value"><input class="form-input" id="sync-device-token" type="password" placeholder="secret" style="width:200px;"></span></div>
      <div class="settings-row"><span class="settings-label">Имя устройства</span>
        <span class="settings-value"><input class="form-input" id="sync-device-name" type="text" value="${escapeHtml(status.device_name)}" placeholder="MacBook" style="width:200px;"></span></div>
      <div class="settings-row"><span class="settings-label">Device ID</span>
        <span class="settings-value"><span class="settings-hint">${status.site_id || '—'}</span></span></div>
      <div class="settings-row"><span class="settings-label">Последняя синхронизация</span>
        <span class="settings-value"><span class="settings-hint">${status.last_sync || 'никогда'}</span></span></div>
      <div class="settings-row"><span class="settings-label">Ожидающие изменения</span>
        <span class="settings-value"><span class="settings-hint">${status.pending_changes}</span></span></div>
      <div class="settings-row" style="gap:var(--space-2);justify-content:flex-end;">
        <button class="btn-smallall" id="sync-save-btn">Сохранить</button>
        <button class="btn-primary" id="sync-now-btn">Синхронизировать</button>
      </div>
    </div>`;
}

export function wireSyncControls(el) {
  el.querySelector('#sync-save-btn')?.addEventListener('click', async () => {
    const enabled = el.querySelector('#sync-enabled')?.checked || false;
    const relayUrl = el.querySelector('#sync-relay-url')?.value || '';
    const deviceToken = el.querySelector('#sync-device-token')?.value || '';
    const deviceName = el.querySelector('#sync-device-name')?.value || '';
    try {
      await invoke('set_sync_config', { enabled, relayUrl, deviceToken, deviceName });
    } catch(e) { console.error('sync config save:', e); }
  });
  el.querySelector('#sync-now-btn')?.addEventListener('click', async () => {
    const btn = el.querySelector('#sync-now-btn');
    btn.textContent = 'Синхронизация…';
    btn.disabled = true;
    try {
      const result = await invoke('sync_now');
      btn.textContent = 'Готово!';
      setTimeout(() => { btn.textContent = 'Синхронизировать'; btn.disabled = false; }, 2000);
    } catch(e) {
      btn.textContent = 'Ошибка';
      console.error('sync:', e);
      setTimeout(() => { btn.textContent = 'Синхронизировать'; btn.disabled = false; }, 3000);
    }
  });
}
