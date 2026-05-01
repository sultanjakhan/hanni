// share-modal.js — Modal for managing public share-links of a tab.

import { invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { openCloudShareModal } from './cloud-share-modal.js';

const PERM_LABELS = { view: 'Просмотр', add: 'Добавление', edit: 'Редактирование', delete: 'Удаление', comment: 'Комментарии' };
const SCOPE_LABELS = {
  food: { all: 'Вся еда', recipes: 'Рецепты', products: 'Продукты (каталог)', fridge: 'Холодильник', meal_plan: 'План меню', memory: 'Память (что не есть)' },
};
const LIFETIME_LABELS = { once: 'Одноразовая', permanent: 'Постоянная', expires: 'С экспирацией' };
const EXPIRY_OPTIONS = [
  { v: 3600, l: '1 час' },
  { v: 86400, l: '24 часа' },
  { v: 604800, l: '7 дней' },
  { v: 2592000, l: '30 дней' },
];

export function openShareModal(tabId) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal share-modal">
    <div class="modal-title" style="display:flex;align-items:center;gap:8px;justify-content:space-between">
      <span>Общий доступ · ${escapeHtml(tabId)} <span id="share-tunnel-status" class="share-tunnel-dot" title="Статус туннеля"></span></span>
      <button class="share-icon-btn" id="share-cloud-cfg" title="Облачный share (Firebase)" style="font-size:16px">☁</button>
    </div>
    <div id="share-body"><div style="color:var(--text-muted);padding:12px 0;">Загрузка…</div></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="share-close">Закрыть</button>
      <button class="btn-primary" id="share-new">+ Новая ссылка</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#share-close').addEventListener('click', () => overlay.remove());
  overlay.querySelector('#share-new').addEventListener('click', () => showCreateForm(overlay, tabId));
  overlay.querySelector('#share-cloud-cfg').addEventListener('click', () => openCloudShareModal());

  refreshList(overlay, tabId);
  updateTunnelStatus(overlay);
}

async function updateTunnelStatus(overlay) {
  try {
    const st = await invoke('tunnel_status');
    const dot = overlay.querySelector('#share-tunnel-status');
    if (!dot) return;
    if (st.running && st.url) {
      dot.className = 'share-tunnel-dot on';
      dot.title = `Туннель активен: ${st.url}`;
    } else {
      dot.className = 'share-tunnel-dot off';
      dot.title = st.error || 'Туннель не запущен (поднимется при создании первой ссылки)';
    }
  } catch {}
}

async function refreshList(overlay, tabId) {
  const body = overlay.querySelector('#share-body');
  try {
    const links = await invoke('list_share_links', { tab: tabId });
    body.innerHTML = renderList(links, tabId);
    body.querySelectorAll('[data-copy]').forEach(btn => btn.addEventListener('click', async () => {
      const url = btn.dataset.copy;
      try { await navigator.clipboard.writeText(url); btn.textContent = '✓'; setTimeout(() => { btn.textContent = '📋'; }, 1200); }
      catch { alert('Не удалось скопировать'); }
    }));
    body.querySelectorAll('[data-revoke]').forEach(btn => btn.addEventListener('click', async () => {
      const id = parseInt(btn.dataset.revoke);
      if (!(await confirmModal('Отозвать эту ссылку? Действие необратимо.'))) return;
      await invoke('revoke_share_link', { id });
      refreshList(overlay, tabId);
    }));
    body.querySelectorAll('[data-delete]').forEach(btn => btn.addEventListener('click', async () => {
      const id = parseInt(btn.dataset.delete);
      if (!(await confirmModal('Удалить ссылку и всю её историю навсегда?'))) return;
      await invoke('delete_share_link', { id });
      refreshList(overlay, tabId);
    }));
    body.querySelectorAll('[data-cloud-push]').forEach(btn => btn.addEventListener('click', async () => {
      const id = parseInt(btn.dataset.cloudPush);
      const orig = btn.textContent;
      btn.textContent = '↑ Пушим…'; btn.disabled = true;
      try {
        const out = await invoke('cloud_share_push', { shareId: id });
        const w = out.written || {};
        const total = Object.values(w).reduce((a, b) => a + (typeof b === 'number' ? b : 0), 0);
        btn.textContent = `✓ ${total} строк`;
        setTimeout(() => { btn.textContent = orig; btn.disabled = false; }, 2500);
      } catch (e) {
        btn.textContent = orig; btn.disabled = false;
        alert('Облачный push не удался: ' + (e?.message || e) + '\n\nНастрой через ☁ в шапке.');
      }
    }));
    body.querySelectorAll('[data-qr]').forEach(btn => btn.addEventListener('click', () => {
      const row = btn.dataset.row;
      const qrBox = body.querySelector(`#share-qr-${row}`);
      if (!qrBox) return;
      if (qrBox.style.display === 'none') {
        const url = btn.dataset.qr;
        const src = `https://api.qrserver.com/v1/create-qr-code/?size=180x180&margin=1&data=${encodeURIComponent(url)}`;
        qrBox.innerHTML = `<img alt="QR" width="180" height="180" src="${src}">`;
        qrBox.style.display = '';
      } else {
        qrBox.style.display = 'none';
        qrBox.innerHTML = '';
      }
    }));
  } catch (err) {
    body.innerHTML = `<div class="share-err">Ошибка: ${escapeHtml(String(err))}</div>`;
  }
}

function renderList(links, tabId) {
  if (!links.length) return '<div style="color:var(--text-muted);padding:12px 0;">Пока нет активных ссылок</div>';
  const scopeMap = SCOPE_LABELS[tabId] || {};
  return links.map(l => {
    const isActive = !l.revoked_at;
    const perms = (l.permissions || []).map(p => `<span class="share-chip">${escapeHtml(PERM_LABELS[p] || p)}</span>`).join('');
    const scopeLabel = scopeMap[l.scope] || l.scope;
    const url = l.url || '';
    const statusTxt = !isActive ? 'Отозвана' : (url ? 'Активна' : 'Туннель не запущен');
    return `<div class="share-row ${isActive ? '' : 'share-row-revoked'}">
      <div class="share-row-top">
        <div class="share-row-label">${escapeHtml(l.label || 'Без подписи')} · <span style="color:var(--text-muted);font-weight:400">${escapeHtml(scopeLabel)}</span></div>
        <div class="share-row-status">${escapeHtml(statusTxt)} · ${l.used_count} открытий</div>
      </div>
      <div class="share-row-perms">${perms}</div>
      ${url ? `<div class="share-row-url">
        <input class="share-url-input" readonly value="${escapeHtml(url)}">
        <button class="share-icon-btn" data-copy="${escapeHtml(url)}" title="Скопировать">📋</button>
        <button class="share-icon-btn" data-qr="${escapeHtml(url)}" data-row="${l.id}" title="QR-код">▦</button>
      </div>
      <div class="share-qr" id="share-qr-${l.id}" style="display:none"></div>` : ''}
      <div class="share-row-actions">
        ${isActive ? `<button class="btn-link" data-cloud-push="${l.id}" title="Залить snapshot в Firebase">☁ Push</button>` : ''}
        ${isActive ? `<button class="btn-link-danger" data-revoke="${l.id}">Отозвать</button>` : ''}
        <button class="btn-link-danger" data-delete="${l.id}">Удалить навсегда</button>
      </div>
    </div>`;
  }).join('');
}

function showCreateForm(overlay, tabId) {
  const body = overlay.querySelector('#share-body');
  const scopeMap = SCOPE_LABELS[tabId] || { all: 'Вся вкладка' };
  const scopeRadios = Object.entries(scopeMap).map(([v, l], i) =>
    `<label class="share-radio"><input type="radio" name="scope" value="${v}" ${i === 0 ? 'checked' : ''}>${escapeHtml(l)}</label>`
  ).join('');
  const permChecks = Object.entries(PERM_LABELS).map(([v, l]) =>
    `<label class="share-check"><input type="checkbox" name="perm" value="${v}" ${v === 'view' ? 'checked' : ''}>${escapeHtml(l)}</label>`
  ).join('');
  const lifetimeRadios = Object.entries(LIFETIME_LABELS).map(([v, l]) =>
    `<label class="share-radio"><input type="radio" name="lifetime" value="${v}" ${v === 'permanent' ? 'checked' : ''}>${escapeHtml(l)}</label>`
  ).join('');
  const expiresOpts = EXPIRY_OPTIONS.map(o => `<option value="${o.v}">${escapeHtml(o.l)}</option>`).join('');

  body.innerHTML = `<div class="share-form">
    <div class="form-group"><label class="form-label">Подпись</label>
      <input class="form-input" id="s-label" placeholder="Для жены, для мамы...">
    </div>
    <div class="form-group"><label class="form-label">Что шарим</label>
      <div class="share-group">${scopeRadios}</div>
    </div>
    <div class="form-group"><label class="form-label">Права</label>
      <div class="share-group">${permChecks}</div>
    </div>
    <div class="form-group"><label class="form-label">Срок</label>
      <div class="share-group">${lifetimeRadios}</div>
      <select class="form-input" id="s-expiry" style="margin-top:6px;display:none">${expiresOpts}</select>
    </div>
    <div id="s-msg"></div>
  </div>`;

  const expirySel = body.querySelector('#s-expiry');
  body.querySelectorAll('input[name="lifetime"]').forEach(r => r.addEventListener('change', () => {
    expirySel.style.display = r.checked && r.value === 'expires' ? '' : expirySel.style.display;
    expirySel.style.display = body.querySelector('input[name="lifetime"]:checked').value === 'expires' ? '' : 'none';
  }));

  const actions = overlay.querySelector('.modal-actions');
  actions.innerHTML = `
    <button class="btn-secondary" id="s-cancel">Отмена</button>
    <button class="btn-primary" id="s-create">Создать ссылку</button>`;
  actions.querySelector('#s-cancel').addEventListener('click', () => { resetFooter(overlay, tabId); refreshList(overlay, tabId); });
  actions.querySelector('#s-create').addEventListener('click', () => submitCreate(overlay, tabId));
}

async function submitCreate(overlay, tabId) {
  const body = overlay.querySelector('#share-body');
  const label = body.querySelector('#s-label').value.trim();
  const scope = body.querySelector('input[name="scope"]:checked')?.value || 'all';
  const lifetime = body.querySelector('input[name="lifetime"]:checked')?.value || 'permanent';
  const permissions = Array.from(body.querySelectorAll('input[name="perm"]:checked')).map(c => c.value);
  const msg = body.querySelector('#s-msg');

  if (!permissions.length) { msg.innerHTML = '<div class="share-err">Выберите хотя бы одно право</div>'; return; }

  let expires_at = null;
  if (lifetime === 'expires') {
    const secs = parseInt(body.querySelector('#s-expiry').value);
    expires_at = new Date(Date.now() + secs * 1000).toISOString();
  }

  msg.innerHTML = '<div style="color:var(--text-muted);">Создаём ссылку, поднимаем туннель…</div>';
  try {
    const link = await invoke('create_share_link', { tab: tabId, scope, permissions, label, lifetime, expiresAt: expires_at });
    if (!link.url) {
      msg.innerHTML = `<div class="share-err">Ссылка создана, но туннель не поднялся. Установите cloudflared: <code>brew install cloudflared</code> и нажмите «Обновить».</div>`;
    }
    resetFooter(overlay, tabId);
    refreshList(overlay, tabId);
  } catch (err) {
    msg.innerHTML = `<div class="share-err">Ошибка: ${escapeHtml(String(err))}</div>`;
  }
}

function resetFooter(overlay, tabId) {
  const actions = overlay.querySelector('.modal-actions');
  actions.innerHTML = `
    <button class="btn-secondary" id="share-close">Закрыть</button>
    <button class="btn-primary" id="share-new">+ Новая ссылка</button>`;
  actions.querySelector('#share-close').addEventListener('click', () => overlay.remove());
  actions.querySelector('#share-new').addEventListener('click', () => showCreateForm(overlay, tabId));
}
