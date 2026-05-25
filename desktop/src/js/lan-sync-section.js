// lan-sync-section.js — UI handlers for LAN/Tailscale sync inside the
// cloud-share-modal. Lets the user enter the peer IP, shared key, toggle
// enabled, and trigger a manual sync — without touching SQL.
import { invoke } from './state.js';

export async function attachLanSync(overlay) {
  let cfg = { peer: '', key: '', enabled: false };
  try { cfg = await invoke('lan_sync_get_config'); } catch {}

  const peerEl = overlay.querySelector('#ls-peer');
  const keyEl = overlay.querySelector('#ls-key');
  const enEl = overlay.querySelector('#ls-enabled');
  const msgEl = overlay.querySelector('#ls-msg');
  if (!peerEl || !keyEl || !enEl) return () => {};

  peerEl.value = cfg.peer || '';
  keyEl.value = cfg.key || '';
  enEl.checked = !!cfg.enabled;

  async function save() {
    msgEl.textContent = 'Сохраняю…';
    msgEl.style.color = 'var(--text-muted)';
    try {
      await invoke('lan_sync_set_config', {
        peer: peerEl.value.trim(),
        key: keyEl.value.trim(),
        enabled: !!enEl.checked,
      });
      msgEl.textContent = '✓ Сохранено';
      msgEl.style.color = 'var(--color-green)';
    } catch (e) {
      msgEl.textContent = 'Ошибка: ' + (e?.message || e);
      msgEl.style.color = 'var(--color-red)';
    }
  }
  overlay.querySelector('#ls-save')?.addEventListener('click', save);

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
