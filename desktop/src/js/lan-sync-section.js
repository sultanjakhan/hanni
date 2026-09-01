// lan-sync-section.js — UI handlers for LAN/Tailscale sync inside the
// cloud-share-modal. Lets the user enter the peer IP, shared key, toggle
// enabled, and trigger a manual sync — without touching SQL.
import { invoke } from './state.js';

export async function attachLanSync(overlay) {
  let cfg = { peer: '', key_set: false, enabled: false };
  try { cfg = await invoke('lan_sync_get_config'); } catch {}

  const peerEl = overlay.querySelector('#ls-peer');
  const keyEl = overlay.querySelector('#ls-key');
  const enEl = overlay.querySelector('#ls-enabled');
  const msgEl = overlay.querySelector('#ls-msg');
  if (!peerEl || !keyEl || !enEl) return () => {};

  peerEl.value = cfg.peer || '';
  keyEl.value = '';
  keyEl.placeholder = cfg.key_set ? 'Ключ сохранён — оставьте пустым, чтобы не менять' : 'Введите новый ключ';
  enEl.checked = !!cfg.enabled;

  async function save() {
    msgEl.textContent = 'Сохраняю…';
    msgEl.style.color = 'var(--text-muted)';
    try {
      await invoke('lan_sync_set_config', {
        peer: peerEl.value.trim(),
        key: keyEl.value.trim() || null,
        clearKey: false,
        enabled: !!enEl.checked,
      });
      if (keyEl.value.trim()) {
        keyEl.value = '';
        keyEl.placeholder = 'Ключ сохранён — оставьте пустым, чтобы не менять';
      }
      msgEl.textContent = '✓ Сохранено';
      msgEl.style.color = 'var(--color-green)';
    } catch (e) {
      msgEl.textContent = 'Ошибка: ' + (e?.message || e);
      msgEl.style.color = 'var(--color-red)';
    }
  }
  overlay.querySelector('#ls-save')?.addEventListener('click', save);

  overlay.querySelector('#ls-clear-key')?.addEventListener('click', async () => {
    msgEl.textContent = 'Удаляю ключ…';
    msgEl.style.color = 'var(--text-muted)';
    try {
      await invoke('lan_sync_set_config', {
        peer: peerEl.value.trim(),
        key: null,
        clearKey: true,
        enabled: false,
      });
      keyEl.value = '';
      keyEl.placeholder = 'Введите новый ключ';
      enEl.checked = false;
      msgEl.textContent = '✓ Ключ удалён, авто-синхронизация выключена';
      msgEl.style.color = 'var(--color-green)';
    } catch (e) {
      msgEl.textContent = 'Ошибка: ' + (e?.message || e);
      msgEl.style.color = 'var(--color-red)';
    }
  });

  overlay.querySelector('#ls-sync-now')?.addEventListener('click', async () => {
    msgEl.textContent = 'Sync через LAN…';
    msgEl.style.color = 'var(--text-muted)';
    try {
      const r = await invoke('lan_sync_now');
      msgEl.textContent = `✓ Отправлено: ${r.sent || 0}, получено: ${r.received || 0}, удалено: ${r.deletes || 0}`;
      msgEl.style.color = 'var(--color-green)';
    } catch (e) {
      msgEl.textContent = 'Ошибка: ' + (e?.message || e);
      msgEl.style.color = 'var(--color-red)';
    }
  });

  return () => {};
}
