// cloud-share-modal.js — Settings modal for Firebase cloud-share mirror.
// Lets the user paste the Web SDK config + service-account JSON, and test
// the connection. Opened from share-modal via the ☁ button.

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export async function openCloudShareModal() {
  let cfg = null;
  try { cfg = await invoke('cloud_share_get_config'); } catch {}

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:90vh;overflow-y:auto">
    <div class="modal-title">☁ Облачный share (Firebase)</div>
    <p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px">
      Когда настроено, ссылки будут зеркалиться в Firestore и работать
      даже если Hanni закрыт. Подробности — <code>firebase/README.md</code>.
    </p>

    <div class="form-group">
      <label class="form-label">Project ID</label>
      <input class="form-input" id="cs-project" placeholder="hanni-share-abc12345"
             value="${escapeHtml(cfg?.project_id || '')}">
    </div>

    <div class="form-group">
      <label class="form-label">Web API key</label>
      <input class="form-input" id="cs-apikey" placeholder="AIza..."
             value="${escapeHtml(cfg?.api_key || '')}">
    </div>

    <div class="form-group">
      <label class="form-label">Service account JSON
        <span style="color:var(--text-muted);font-size:11px">
          Firebase Console → Project settings → Service accounts → Generate new private key
        </span>
      </label>
      <textarea class="form-input" id="cs-sa" rows="6" spellcheck="false"
        style="font-family:ui-monospace,Menlo,monospace;font-size:11px"
        placeholder='{"type":"service_account","project_id":"...","private_key":"-----BEGIN PRIVATE KEY-----\\n..."}'>${cfg?.service_account_json === '<set>' ? '' : ''}</textarea>
      <div style="color:var(--text-muted);font-size:11px;margin-top:4px">
        ${cfg?.service_account_json === '<set>'
          ? '🔒 Сохранён ранее. Оставьте пустым чтобы не менять.'
          : '🔓 Не сохранён.'}
      </div>
    </div>

    <div class="form-group">
      <label class="form-label">Owner UID
        <span style="color:var(--text-muted);font-size:11px">генерится один раз, не меняется</span>
      </label>
      <input class="form-input" id="cs-owner" readonly disabled
             value="${escapeHtml(cfg?.owner_uid || '— ещё не сохранён —')}">
    </div>

    <div id="cs-msg" style="min-height:18px;font-size:13px"></div>

    <hr style="border:none;border-top:1px solid var(--border-subtle);margin:16px 0">
    <div style="font-weight:600;margin-bottom:8px">🔄 Синхронизация устройств (Mac ↔ Mobile)</div>
    <p style="color:var(--text-secondary);font-size:12px;margin:0 0 8px">
      Push — отправляет CRDT-changes этого устройства в Firestore. Pull — забирает changes других устройств с тем же Owner UID и применяет.
    </p>
    <div id="cs-owner-status" style="font-size:12px;color:var(--text-muted);margin-bottom:8px"></div>
    <div style="display:flex;gap:8px;flex-wrap:wrap">
      <button class="btn-secondary" id="cs-owner-push">⬆ Push</button>
      <button class="btn-secondary" id="cs-owner-pull">⬇ Pull</button>
      <button class="btn-primary"   id="cs-owner-sync">🔄 Sync now</button>
    </div>
    <div id="cs-owner-msg" style="min-height:18px;font-size:13px;margin-top:8px"></div>

    <div class="modal-actions" style="flex-wrap:wrap;gap:8px;margin-top:16px">
      <button class="btn-secondary" id="cs-close">Закрыть</button>
      <button class="btn-secondary" id="cs-dry">Dry-run push #1</button>
      <button class="btn-primary"   id="cs-save">Сохранить</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#cs-close').onclick = () => overlay.remove();

  const msg = overlay.querySelector('#cs-msg');
  function showOk(t)  { msg.innerHTML = `<div style="color:var(--color-green)">${escapeHtml(t)}</div>`; }
  function showErr(t) { msg.innerHTML = `<div style="color:var(--color-red)">${escapeHtml(t)}</div>`; }
  function showHint(t){ msg.innerHTML = `<div style="color:var(--text-muted)">${escapeHtml(t)}</div>`; }

  overlay.querySelector('#cs-save').onclick = async () => {
    const projectId = overlay.querySelector('#cs-project').value.trim();
    const apiKey    = overlay.querySelector('#cs-apikey').value.trim();
    const sa        = overlay.querySelector('#cs-sa').value.trim();
    if (!projectId || !apiKey) {
      showErr('Project ID и API key обязательны.'); return;
    }
    showHint('Сохраняем…');
    try {
      const args = { projectId, apiKey };
      if (sa) args.serviceAccountJson = sa;
      const saved = await invoke('cloud_share_set_config', args);
      overlay.querySelector('#cs-owner').value = saved.owner_uid;
      showOk('✓ Сохранено. Owner UID: ' + saved.owner_uid);
    } catch (e) { showErr('Ошибка: ' + (e?.message || e)); }
  };

  // Owner sync (mac ↔ mobile)
  const ownerMsg = overlay.querySelector('#cs-owner-msg');
  const ownerStatus = overlay.querySelector('#cs-owner-status');
  function ownerOk(t)  { ownerMsg.innerHTML = `<div style="color:var(--color-green)">${escapeHtml(t)}</div>`; }
  function ownerErr(t) { ownerMsg.innerHTML = `<div style="color:var(--color-red)">${escapeHtml(t)}</div>`; }
  function ownerHint(t){ ownerMsg.innerHTML = `<div style="color:var(--text-muted)">${escapeHtml(t)}</div>`; }
  async function refreshOwnerStatus() {
    try {
      const st = await invoke('cloud_owner_status');
      ownerStatus.textContent = `site_id: ${(st.site_id || '').slice(0, 12)}…  ·  pending push: ${st.pending_changes}  ·  last pull: ${st.last_pull_ts || 'никогда'}`;
    } catch (e) { ownerStatus.textContent = 'статус: ' + (e?.message || e); }
  }
  refreshOwnerStatus();
  overlay.querySelector('#cs-owner-push').onclick = async () => {
    ownerHint('Пушим changes…');
    try {
      const out = await invoke('cloud_owner_push');
      ownerOk(`✓ Push: ${out.pushed} changes (db_version=${out.db_version})`);
      refreshOwnerStatus();
    } catch (e) { ownerErr('Ошибка push: ' + (e?.message || e)); }
  };
  overlay.querySelector('#cs-owner-pull').onclick = async () => {
    ownerHint('Тянем changes…');
    try {
      const out = await invoke('cloud_owner_pull');
      ownerOk(`✓ Pull: applied ${out.applied}, skipped own ${out.skipped_own}, total in cloud ${out.total}`);
      refreshOwnerStatus();
    } catch (e) { ownerErr('Ошибка pull: ' + (e?.message || e)); }
  };
  overlay.querySelector('#cs-owner-sync').onclick = async () => {
    ownerHint('Sync (push + pull)…');
    try {
      const push = await invoke('cloud_owner_push');
      const pull = await invoke('cloud_owner_pull');
      ownerOk(`✓ Sync: pushed ${push.pushed}, pulled ${pull.applied} (skipped own ${pull.skipped_own})`);
      refreshOwnerStatus();
    } catch (e) { ownerErr('Ошибка sync: ' + (e?.message || e)); }
  };

  overlay.querySelector('#cs-dry').onclick = async () => {
    showHint('Считаем snapshot для shareId=1…');
    try {
      const out = await invoke('cloud_share_push', { shareId: 1, dryRun: true });
      const c = out.counts || {};
      const total = Object.values(c).reduce((a, b) => a + b, 0);
      showOk(`✓ Dry-run OK · ${total} строк (recipes ${c.recipes ?? 0} · ingredients ${c.recipe_ingredients ?? 0} · catalog ${c.ingredient_catalog ?? 0} · products ${c.products ?? 0} · blacklist ${c.food_blacklist ?? 0} · plan ${c.meal_plan ?? 0})`);
    } catch (e) { showErr('Ошибка: ' + (e?.message || e)); }
  };
}
