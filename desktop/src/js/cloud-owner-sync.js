// cloud-owner-sync.js — handlers for the owner-sync section of the
// cloud-share modal: status refresh, push/pull/sync buttons, owner UID
// copy/import, auto-sync toggle, and the live tick listener.

import { invoke, listen } from './state.js';
import { escapeHtml } from './utils.js';

export function attachOwnerSync(overlay, autoCfg) {
  const ownerMsg    = overlay.querySelector('#cs-owner-msg');
  const ownerStatus = overlay.querySelector('#cs-owner-status');
  const autoEnabled = overlay.querySelector('#cs-auto-enabled');
  const autoSecs    = overlay.querySelector('#cs-auto-secs');
  const autoTick    = overlay.querySelector('#cs-auto-tick');

  const ok   = (t) => ownerMsg.innerHTML = `<div style="color:var(--color-green)">${escapeHtml(t)}</div>`;
  const err  = (t) => ownerMsg.innerHTML = `<div style="color:var(--color-red)">${escapeHtml(t)}</div>`;
  const hint = (t) => ownerMsg.innerHTML = `<div style="color:var(--text-muted)">${escapeHtml(t)}</div>`;

  async function refreshStatus() {
    try {
      const st = await invoke('cloud_owner_status');
      ownerStatus.textContent = `site_id: ${(st.site_id || '').slice(0, 12)}…  ·  pending push: ${st.pending_changes}  ·  last pull: ${st.last_pull_ts || 'никогда'}`;
    } catch (e) { ownerStatus.textContent = 'статус: ' + (e?.message || e); }
  }
  refreshStatus();

  overlay.querySelector('#cs-owner-push').onclick = async () => {
    hint('Пушим changes…');
    try {
      const out = await invoke('cloud_owner_push');
      ok(`✓ Push: ${out.pushed} changes (db_version=${out.db_version})`);
      refreshStatus();
    } catch (e) { err('Ошибка push: ' + (e?.message || e)); }
  };
  overlay.querySelector('#cs-owner-pull').onclick = async () => {
    hint('Тянем changes…');
    try {
      const out = await invoke('cloud_owner_pull');
      ok(`✓ Pull: applied ${out.applied}, skipped own ${out.skipped_own}, total ${out.total}`);
      refreshStatus();
    } catch (e) { err('Ошибка pull: ' + (e?.message || e)); }
  };
  overlay.querySelector('#cs-owner-sync').onclick = async () => {
    hint('Sync (push + pull)…');
    try {
      const push = await invoke('cloud_owner_push');
      const pull = await invoke('cloud_owner_pull');
      ok(`✓ Sync: pushed ${push.pushed}, pulled ${pull.applied} (skipped own ${pull.skipped_own})`);
      refreshStatus();
    } catch (e) { err('Ошибка sync: ' + (e?.message || e)); }
  };

  overlay.querySelector('#cs-owner-copy').onclick = async () => {
    const uid = overlay.querySelector('#cs-owner').value.trim();
    if (!uid || uid.startsWith('—')) { err('Сначала сохраните service-account.'); return; }
    try { await navigator.clipboard.writeText(uid); ok('UID скопирован: ' + uid); }
    catch (e) { err('Не удалось скопировать: ' + (e?.message || e)); }
  };
  overlay.querySelector('#cs-owner-import-btn').onclick = async () => {
    const newUid = overlay.querySelector('#cs-owner-import').value.trim();
    if (!newUid) { err('Вставьте UID с другого устройства.'); return; }
    if (!confirm(`Сменить Owner UID на "${newUid}"?\n\nPush/pull-метки сбросятся — следующий sync перепушит и перетянет всё с нуля.`)) return;
    try {
      const applied = await invoke('cloud_owner_set_uid', { uid: newUid });
      overlay.querySelector('#cs-owner').value = applied;
      overlay.querySelector('#cs-owner-import').value = '';
      ok('✓ Owner UID обновлён: ' + applied);
      refreshStatus();
    } catch (e) { err('Ошибка: ' + (e?.message || e)); }
  };

  async function saveAuto() {
    const enabled = !!autoEnabled.checked;
    const intervalSecs = Math.max(30, Math.min(600, Number(autoSecs.value) || 60));
    autoSecs.value = intervalSecs;
    try {
      await invoke('cloud_owner_set_auto', { enabled, intervalSecs });
      autoTick.textContent = enabled
        ? `Авто-sync включён, каждые ${intervalSecs} сек. Жду первый тик…`
        : 'Авто-sync выключен.';
    } catch (e) { err('Не удалось сохранить авто-sync: ' + (e?.message || e)); }
  }
  autoEnabled.addEventListener('change', saveAuto);
  autoSecs.addEventListener('change', saveAuto);
  autoTick.textContent = autoCfg.enabled
    ? `Авто-sync включён, каждые ${autoCfg.interval_secs} сек.`
    : 'Авто-sync выключен.';

  let unlisten = null;
  listen('cloud-owner-sync-tick', (ev) => {
    const p = ev?.payload || {};
    const t = (p.ts || '').slice(11, 19);
    if (p.ok) {
      const pushed = p.push?.pushed ?? 0;
      const pulled = p.pull?.applied ?? 0;
      autoTick.textContent = `✓ ${t} · pushed ${pushed} · pulled ${pulled}`;
      refreshStatus();
    } else {
      autoTick.textContent = `⚠ ${t} · ${p.error || 'sync failed'}`;
    }
  }).then((fn) => { unlisten = fn; }).catch(() => {});

  return () => { if (typeof unlisten === 'function') { try { unlisten(); } catch {} } };
}
