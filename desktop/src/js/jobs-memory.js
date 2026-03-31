// jobs-memory.js — 4 inner tabs for Jobs Memory pane: Resume, Sources, Positions, Cover Letter
import { invoke, S } from './state.js';
import { renderResume } from './jobs-resume.js';
import { renderSources } from './jobs-sources.js';
import { showEditModal, EDIT_ICON } from './jobs-edit.js';
import { escapeHtml } from './utils.js';

const INNER_TABS = [
  { id: 'resume', label: 'Резюме' },
  { id: 'sources', label: 'Источники' },
  { id: 'positions', label: 'Позиции' },
  { id: 'cover', label: 'Письма' },
];

export async function renderJobsMemory(paneEl) {
  if (!S._jobsMemTab) S._jobsMemTab = 'resume';
  const active = S._jobsMemTab;

  const tabsHtml = INNER_TABS.map(t =>
    `<div class="jm-tab${t.id === active ? ' active' : ''}" data-jm="${t.id}">${t.label}</div>`
  ).join('');

  paneEl.innerHTML = `<div class="jm-tabs-bar">${tabsHtml}</div><div class="jm-pane"></div>`;

  paneEl.querySelectorAll('.jm-tab').forEach(tab => {
    tab.addEventListener('click', async () => {
      S._jobsMemTab = tab.dataset.jm;
      await renderJobsMemory(paneEl);
    });
  });

  const pane = paneEl.querySelector('.jm-pane');
  switch (active) {
    case 'resume': await renderResume(pane); break;
    case 'sources': await renderSources(pane); break;
    case 'positions': await renderPositions(pane); break;
    case 'cover': await renderGenericStore(pane, 'jobs_cover', 'Письма', ['Общее письмо', 'PM позиция', 'Data позиция']); break;
  }
}

async function renderPositions(el) {
  let entries = [];
  try { entries = await invoke('memory_list', { category: 'jobs_positions', limit: 100 }); } catch {}

  if (entries.length === 0) {
    el.innerHTML = `<div class="jm-empty">
      <div class="jm-empty-icon">🎯</div>
      <div class="jm-empty-title">Позиции не заданы</div>
      <div class="jm-empty-desc">Укажите какие роли вы ищете</div>
      <button class="btn btn-sm btn-primary jm-seed-pos-btn">Заполнить</button>
    </div>`;
    el.querySelector('.jm-seed-pos-btn')?.addEventListener('click', async () => {
      await seedPositions();
      await renderPositions(el);
    });
    return;
  }

  const global = entries.find(e => e.key === '_settings');
  const settings = global ? JSON.parse(global.value || '{}') : {};
  const roles = entries.filter(e => e.key !== '_settings');

  el.innerHTML = `<div class="jm-resume" id="jm-pos-root"></div>`;
  const root = el.querySelector('#jm-pos-root');
  let html = '';

  if (Object.keys(settings).length) {
    html += `<div class="jm-section"><div class="jm-section-header"><span class="jm-section-title">Общие параметры</span><button class="jm-edit-btn" data-edit="settings">${EDIT_ICON}</button></div>
      <div class="jm-contact-grid">
        ${sf('Область', settings.area)}${sf('Зарплата', settings.salary)}${sf('Опыт', settings.experience)}
        ${sf('Формат', settings.format)}${sf('Город', settings.city)}
      </div></div>`;
  }

  const cards = roles.map(e => {
    const d = JSON.parse(e.value || '{}');
    return `<div class="jm-card"><div class="jm-card-top">
      <span class="jm-card-title">${escapeHtml(d.title || e.key)}</span>
      <button class="jm-edit-btn" data-edit="role" data-key="${escapeHtml(e.key)}">${EDIT_ICON}</button>
    </div>${d.priority ? `<div class="jm-card-sub">Приоритет: ${escapeHtml(d.priority)}</div>` : ''}</div>`;
  }).join('');
  html += `<div class="jm-section"><div class="jm-section-header"><span class="jm-section-title">Роли</span><button class="btn btn-sm jm-add-role-btn">+ Добавить</button></div>${cards}</div>`;

  root.innerHTML = html;

  // Edit settings
  root.querySelector('[data-edit="settings"]')?.addEventListener('click', () => {
    showEditModal('jobs_positions', '_settings', {
      area: { label: 'Область', value: settings.area },
      salary: { label: 'Зарплата', value: settings.salary },
      experience: { label: 'Опыт', value: settings.experience },
      format: { label: 'Формат', value: settings.format },
      city: { label: 'Город', value: settings.city },
    }, () => renderPositions(el));
  });

  // Edit roles
  root.querySelectorAll('[data-edit="role"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const key = btn.dataset.key;
      const entry = roles.find(e => e.key === key);
      const d = entry ? JSON.parse(entry.value || '{}') : {};
      showEditModal('jobs_positions', key, {
        title: { label: 'Название', value: d.title },
        priority: { label: 'Приоритет', value: d.priority },
      }, () => renderPositions(el));
    });
  });

  // Add new role
  root.querySelector('.jm-add-role-btn')?.addEventListener('click', () => {
    const nextIdx = roles.length + 1;
    showEditModal('jobs_positions', `role_${nextIdx}`, {
      title: { label: 'Название', value: '' },
      priority: { label: 'Приоритет', value: 'средний' },
    }, () => renderPositions(el));
  });
}

function sf(label, val) {
  return val ? `<div class="jm-contact-field"><span class="jm-contact-label">${label}</span><span class="jm-contact-value">${escapeHtml(val)}</span></div>` : '';
}

async function seedPositions() {
  const settings = { area: 'Все', salary: 'от 100,000 ₸', experience: '0-1 / 1-3 года', format: 'Все', city: 'Любой' };
  await invoke('memory_remember', { category: 'jobs_positions', key: '_settings', value: JSON.stringify(settings) }).catch(() => {});
  const roles = [
    { title: 'Product Manager', priority: 'высокий' },
    { title: 'Project Manager', priority: 'высокий' },
    { title: 'Product Operations / Product Ops', priority: 'высокий' },
    { title: 'Product Analyst', priority: 'средний' },
    { title: 'Associate Product Manager', priority: 'средний' },
  ];
  for (const [i, role] of roles.entries()) {
    await invoke('memory_remember', { category: 'jobs_positions', key: `role_${i + 1}`, value: JSON.stringify(role) }).catch(() => {});
  }
}

async function renderGenericStore(el, category, title, hints) {
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
