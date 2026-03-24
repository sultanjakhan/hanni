// ── db-view/db-filters.js — Filter UI (chips + dropdown) ──

import { S } from '../state.js';
import { escapeHtml } from '../utils.js';
import { getFilterConditions } from './db-type-registry.js';
import { applyFilters, saveFiltersToViewConfig, loadFiltersFromViewConfig } from './db-filter-logic.js';

export { applyFilters, saveFiltersToViewConfig, loadFiltersFromViewConfig };

const ALL_COND_LABELS = {
  contains: 'содержит', not_contains: 'не содержит',
  eq: 'равно', neq: 'не равно',
  starts_with: 'начинается с', ends_with: 'заканчивается на',
  empty: 'пусто', not_empty: 'не пусто',
  gt: '>', lt: '<',
  before: 'до', after: 'после',
  this_week: 'эта неделя', this_month: 'этот месяц',
  last_7_days: 'последние 7 дней', last_30_days: 'последние 30 дней',
};

function condOptionsForType(type) {
  const ids = getFilterConditions(type || 'text');
  return ids.map(id => ({ value: id, label: ALL_COND_LABELS[id] || id }));
}

/** Render filter chip bar above the database view */
export function renderFilterBar(el, tabId, allFields, onApply) {
  const filters = S.dbvFilters[tabId] || [];
  if (filters.length === 0) return;

  const mode = filters._mode || 'and';
  const chips = filters.filter(f => typeof f === 'object' && f.condition).map((f, idx) => {
    const field = allFields.find(p => p.filterKey === f.filterKey);
    const label = field ? field.label : '?';
    const condLabel = ALL_COND_LABELS[f.condition] || f.condition;
    return `<span class="dbv-filter-chip" data-idx="${idx}">
      ${escapeHtml(label)} <em>${condLabel}</em> ${f.value ? escapeHtml(f.value) : ''}
      <span class="dbv-filter-chip-remove" data-remove="${idx}">×</span>
    </span>`;
  }).join('');

  const modeToggle = filters.length > 1
    ? `<button class="dbv-filter-mode-btn">${mode === 'or' ? 'ИЛИ' : 'И'}</button>` : '';

  const bar = document.createElement('div');
  bar.className = 'dbv-filter-bar';
  bar.innerHTML = chips + modeToggle;
  el.prepend(bar);

  bar.querySelectorAll('.dbv-filter-chip-remove').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      S.dbvFilters[tabId].splice(parseInt(btn.dataset.remove), 1);
      saveFiltersToViewConfig(tabId);
      onApply();
    });
  });

  bar.querySelector('.dbv-filter-mode-btn')?.addEventListener('click', () => {
    const f = S.dbvFilters[tabId];
    f._mode = (f._mode || 'and') === 'and' ? 'or' : 'and';
    saveFiltersToViewConfig(tabId);
    onApply();
  });
}

/** Show inline filter dropdown under anchor button */
export function showFilterDropdown(anchorEl, tabId, allFields, onApply) {
  if (allFields.length === 0) return;
  document.querySelectorAll('.dbv-filter-dropdown').forEach(d => d.remove());

  const rect = anchorEl.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'dbv-filter-dropdown';
  dd.style.top = rect.bottom + 4 + 'px';
  dd.style.right = (window.innerWidth - rect.right) + 'px';

  let selField = allFields[0];
  let condOpts = condOptionsForType(selField.type);
  let selCond = condOpts[0];
  let valueStr = '';

  const render = () => {
    condOpts = condOptionsForType(selField.type);
    if (!condOpts.find(c => c.value === selCond.value)) selCond = condOpts[0];
    const fieldOpts = selField.options || [];
    const noValue = ['empty', 'not_empty', 'this_week', 'this_month', 'last_7_days', 'last_30_days'].includes(selCond.value);

    dd.innerHTML = `
      <div class="dbv-fd-row"><div class="dbv-fd-picker" id="dbv-fd-prop">${escapeHtml(selField.label)}<span class="dbv-fd-arrow">▾</span></div></div>
      <div class="dbv-fd-row"><div class="dbv-fd-picker" id="dbv-fd-cond">${escapeHtml(selCond.label)}<span class="dbv-fd-arrow">▾</span></div></div>
      ${!noValue ? `<div class="dbv-fd-row">${fieldOpts.length > 0
        ? `<div class="dbv-fd-picker" id="dbv-fd-val">${valueStr ? escapeHtml(valueStr) : '<span style="color:var(--text-faint)">Выберите...</span>'}<span class="dbv-fd-arrow">▾</span></div>`
        : `<input class="dbv-fd-input" id="dbv-fd-val" placeholder="Значение..." value="${escapeHtml(valueStr)}">`}</div>` : ''}
      <button class="dbv-fd-apply">Применить</button>`;

    dd.querySelector('#dbv-fd-prop')?.addEventListener('click', () => {
      showPickerMenu(dd.querySelector('#dbv-fd-prop'), allFields.map(f => ({ value: f.filterKey, label: f.label })), selField.filterKey, (v) => {
        selField = allFields.find(f => f.filterKey === v) || allFields[0]; valueStr = ''; render();
      });
    });
    dd.querySelector('#dbv-fd-cond')?.addEventListener('click', () => {
      showPickerMenu(dd.querySelector('#dbv-fd-cond'), condOpts, selCond.value, (v) => {
        selCond = condOpts.find(o => o.value === v) || condOpts[0]; render();
      });
    });
    const valEl = dd.querySelector('#dbv-fd-val');
    if (valEl && fieldOpts.length > 0 && valEl.tagName !== 'INPUT') {
      valEl.addEventListener('click', () => { showPickerMenu(valEl, fieldOpts.map(o => ({ value: o, label: o })), valueStr, (v) => { valueStr = v; render(); }); });
    }
    if (valEl?.tagName === 'INPUT') valEl.addEventListener('input', (e) => { valueStr = e.target.value; });

    dd.querySelector('.dbv-fd-apply')?.addEventListener('click', () => {
      if (!S.dbvFilters[tabId]) S.dbvFilters[tabId] = [];
      S.dbvFilters[tabId].push({ filterKey: selField.filterKey, condition: selCond.value, value: valueStr });
      dd.remove(); saveFiltersToViewConfig(tabId); onApply();
    });
  };

  render();
  document.body.appendChild(dd);
  setTimeout(() => {
    const close = (e) => { if (!dd.contains(e.target) && !anchorEl.contains(e.target) && !e.target.closest('.dbv-picker-menu')) { dd.remove(); document.removeEventListener('mousedown', close); } };
    document.addEventListener('mousedown', close);
  }, 10);
}

function showPickerMenu(anchor, options, currentVal, onSelect) {
  document.querySelectorAll('.dbv-picker-menu').forEach(m => m.remove());
  const rect = anchor.getBoundingClientRect();
  const menu = document.createElement('div');
  menu.className = 'dbv-picker-menu';
  menu.style.top = rect.bottom + 2 + 'px';
  menu.style.left = rect.left + 'px';
  menu.style.minWidth = rect.width + 'px';
  menu.innerHTML = options.map(o => `<div class="dbv-picker-item${o.value === currentVal ? ' active' : ''}" data-val="${escapeHtml(o.value)}">${escapeHtml(o.label)}</div>`).join('');
  document.body.appendChild(menu);
  menu.querySelectorAll('.dbv-picker-item').forEach(item => {
    item.addEventListener('click', (e) => { e.stopPropagation(); menu.remove(); onSelect(item.dataset.val); });
  });
  setTimeout(() => { const close = (e) => { if (!menu.contains(e.target)) { menu.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}
