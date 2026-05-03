// Calendar chart — filter state + popup UI.

import { S } from './state.js';
import { escapeHtml } from './utils.js';

export const DEFAULT_TYPES = ['event', 'task', 'schedule'];
const TYPE_LABELS = { event: '📅 События', task: '📝 Задачи', schedule: '🔁 Расписание' };
const CAT_LABELS = {
  'event:general': 'general', 'event:work': 'work', 'event:personal': 'personal',
  'event:health': 'health', 'event:education': 'education', 'event:social': 'social', 'event:travel': 'travel',
  'schedule:health': 'health', 'schedule:sport': 'sport', 'schedule:hygiene': 'hygiene',
  'schedule:home': 'home', 'schedule:practice': 'practice', 'schedule:challenge': 'challenge',
  'schedule:growth': 'growth', 'schedule:work': 'work', 'schedule:other': 'other',
  'task:all': 'все задачи',
};

export function getFilters() {
  if (S.calChartFilters) return S.calChartFilters;
  try { S.calChartFilters = JSON.parse(localStorage.getItem('hanni_cal_chart_filters') || 'null'); } catch {}
  if (!S.calChartFilters || !Array.isArray(S.calChartFilters.types)) {
    S.calChartFilters = { types: [...DEFAULT_TYPES], categories: null };
  }
  return S.calChartFilters;
}
export function setFilters(f) {
  S.calChartFilters = f;
  try { localStorage.setItem('hanni_cal_chart_filters', JSON.stringify(f)); } catch {}
}

export function isFilterActive(filters) {
  const typesDefault = filters.types.length === DEFAULT_TYPES.length && DEFAULT_TYPES.every(t => filters.types.includes(t));
  return !typesDefault || !!filters.categories;
}

function isCatActive(c, filters) {
  return !filters.categories || filters.categories.includes(c);
}

function renderCatGroup(type, filters, availableCats) {
  const groupCats = availableCats.filter(c => c.startsWith(type + ':'));
  if (!groupCats.length) return '';
  const activeCount = groupCats.filter(c => isCatActive(c, filters)).length;
  const allOfTypeOn = activeCount === groupCats.length;
  const chips = groupCats.map(c =>
    `<button class="dev-filter-btn${isCatActive(c, filters) ? ' active' : ''}" data-chart-cat="${c}">${escapeHtml(CAT_LABELS[c] || c)}</button>`
  ).join('');
  return `<details class="cal-chart-filter-group" open>
    <summary class="cal-chart-filter-group-head">
      <span class="cal-chart-filter-group-title">${TYPE_LABELS[type]}</span>
      <span class="cal-chart-filter-group-count">${activeCount}/${groupCats.length}</span>
    </summary>
    <div class="dev-filters cal-chart-filter-group-body">
      <button class="dev-filter-btn${allOfTypeOn ? ' active' : ''}" data-chart-cat-all-type="${type}">Все</button>
      ${chips}
    </div>
  </details>`;
}

export function renderFilterPopup(filters, availableCats) {
  const typeChips = DEFAULT_TYPES.map(t =>
    `<button class="dev-filter-btn${filters.types.includes(t) ? ' active' : ''}" data-chart-type="${t}">${TYPE_LABELS[t]}</button>`
  ).join('');
  const groups = DEFAULT_TYPES.map(t => renderCatGroup(t, filters, availableCats)).filter(Boolean).join('');
  const groupsHtml = groups || '<span class="cal-chart-filter-empty">нет данных</span>';
  return `<div class="cal-chart-filter-popup" hidden>
    <div class="cal-chart-filter-row">
      <span class="cal-chart-filter-title">Типы</span>
      <div class="dev-filters">${typeChips}</div>
    </div>
    <div class="cal-chart-filter-divider">Категории</div>
    <div class="cal-chart-filter-groups">${groupsHtml}</div>
    <div class="cal-chart-filter-actions">
      <button class="btn-sm btn-secondary" data-chart-filter-reset>Сбросить всё</button>
    </div>
  </div>`;
}

function explicitCats(filters, availableCats) {
  return filters.categories ? [...filters.categories] : [...availableCats];
}

export function wireFilters(el, filters, availableCats, rerender) {
  el.querySelectorAll('[data-chart-type]').forEach(btn => {
    btn.addEventListener('click', () => {
      const t = btn.dataset.chartType;
      const next = { ...filters, types: filters.types.includes(t) ? filters.types.filter(x => x !== t) : [...filters.types, t] };
      if (next.types.length === 0) next.types = [...DEFAULT_TYPES];
      setFilters(next); rerender();
    });
  });
  el.querySelectorAll('[data-chart-cat]').forEach(btn => {
    btn.addEventListener('click', () => {
      const c = btn.dataset.chartCat;
      const cur = explicitCats(filters, availableCats);
      const set = new Set(cur);
      if (set.has(c)) set.delete(c); else set.add(c);
      const arr = [...set];
      const isAllOn = arr.length === availableCats.length && availableCats.every(x => set.has(x));
      setFilters({ ...filters, categories: isAllOn ? null : arr });
      rerender();
    });
  });
  el.querySelectorAll('[data-chart-cat-all-type]').forEach(btn => {
    btn.addEventListener('click', () => {
      const t = btn.dataset.chartCatAllType;
      const groupCats = availableCats.filter(c => c.startsWith(t + ':'));
      const cur = explicitCats(filters, availableCats);
      const set = new Set(cur);
      const allOn = groupCats.every(c => set.has(c));
      if (allOn) groupCats.forEach(c => set.delete(c));
      else groupCats.forEach(c => set.add(c));
      const arr = [...set];
      const isAllOn = arr.length === availableCats.length && availableCats.every(x => set.has(x));
      setFilters({ ...filters, categories: isAllOn ? null : arr });
      rerender();
    });
  });
  el.querySelector('[data-chart-filter-reset]')?.addEventListener('click', () => {
    setFilters({ types: [...DEFAULT_TYPES], categories: null }); rerender();
  });
}

export function wireFilterPopup(wrapEl) {
  const btn = wrapEl.querySelector('.cal-chart-filter-btn');
  const pop = wrapEl.querySelector('.cal-chart-filter-popup');
  if (!btn || !pop) return;
  const closeOnOutside = (ev) => {
    if (pop.hidden) return;
    if (wrapEl.contains(ev.target)) return;
    pop.hidden = true;
    document.removeEventListener('mousedown', closeOnOutside);
    document.removeEventListener('keydown', closeOnEsc);
  };
  const closeOnEsc = (ev) => {
    if (ev.key === 'Escape') {
      pop.hidden = true;
      document.removeEventListener('mousedown', closeOnOutside);
      document.removeEventListener('keydown', closeOnEsc);
    }
  };
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    pop.hidden = !pop.hidden;
    if (!pop.hidden) {
      document.addEventListener('mousedown', closeOnOutside);
      document.addEventListener('keydown', closeOnEsc);
    }
  });
}
