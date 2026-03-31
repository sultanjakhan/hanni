// jobs-stores.js — Blacklist and generic store renderers for Jobs Memory
import { invoke } from './state.js';
import { showEditModal, EDIT_ICON } from './jobs-edit.js';
import { escapeHtml } from './utils.js';

export async function renderBlacklist(el) {
  let entries = [];
  try { entries = await invoke('memory_list', { category: 'jobs_blacklist', limit: 100 }); } catch {}

  const items = entries.map(e => `<div class="jm-store-row">
    <span>${escapeHtml(e.value || e.key)}</span>
    <button class="jm-del-bl" data-key="${escapeHtml(e.key)}" title="Удалить">✕</button>
  </div>`).join('');

  el.innerHTML = `<div class="jm-section">
    <div class="jm-section-header"><span class="jm-section-title">Компании в блэклисте</span></div>
    <div class="jm-empty-desc" style="margin-bottom:8px;">Вакансии этих компаний не будут сохраняться</div>
    ${items || '<div class="jm-empty-desc">Пусто</div>'}
    <div style="margin-top:8px;display:flex;gap:6px;">
      <input class="input input-sm jm-bl-input" placeholder="Название компании" style="flex:1;">
      <button class="btn btn-sm btn-primary jm-bl-add">+ Добавить</button>
    </div>
  </div>`;

  el.querySelector('.jm-bl-add')?.addEventListener('click', async () => {
    const inp = el.querySelector('.jm-bl-input');
    const val = inp?.value.trim();
    if (!val) return;
    await invoke('memory_remember', { category: 'jobs_blacklist', key: val.toLowerCase(), value: val }).catch(() => {});
    await renderBlacklist(el);
  });
  el.querySelectorAll('.jm-del-bl').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('memory_forget', { category: 'jobs_blacklist', key: btn.dataset.key }).catch(() => {});
      await renderBlacklist(el);
    });
  });
}

export async function renderGenericStore(el, category, title, hints) {
  let entries = [];
  try { entries = await invoke('memory_list', { category, limit: 100 }); } catch {}

  if (entries.length === 0) {
    el.innerHTML = `<div class="jm-empty">
      <div class="jm-empty-icon">📋</div>
      <div class="jm-empty-title">${title} пусто</div>
      <div class="jm-empty-hints">Примеры: ${hints.map(h => `<span class="jm-hint">${h}</span>`).join('')}</div>
    </div>`;
    return;
  }

  const rows = entries.map(e => `<div class="jm-store-row">
    <div class="jm-store-key">${escapeHtml(e.key)}</div>
    <div class="jm-store-val">${escapeHtml((e.value || '').substring(0, 200))}</div>
    <button class="jm-edit-btn" data-edit="generic" data-cat="${category}" data-key="${escapeHtml(e.key)}">${EDIT_ICON}</button>
  </div>`).join('');

  el.innerHTML = `<div class="jm-store-list">${rows}</div>`;

  el.querySelectorAll('[data-edit="generic"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const entry = entries.find(en => en.key === btn.dataset.key);
      showEditModal(category, btn.dataset.key, {
        key: { label: 'Ключ', value: entry?.key || '' },
        value: { label: 'Значение', value: entry?.value || '', type: 'textarea' },
      }, () => renderGenericStore(el, category, title, hints));
    });
  });
}
