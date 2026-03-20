// ── db-view/db-calc.js — Calculation row (COUNT/SUM/AVG/etc) ──

import { S } from '../state.js';

const CALC_TYPES = [
  { id: 'none', label: 'None' },
  { id: 'count', label: 'Count' },
  { id: 'count_values', label: 'Count values' },
  { id: 'count_empty', label: 'Count empty' },
  { id: 'sum', label: 'Sum' },
  { id: 'avg', label: 'Average' },
  { id: 'min', label: 'Min' },
  { id: 'max', label: 'Max' },
];

/** Get calculation value for a property column */
function calcValue(type, values) {
  const nums = values.map(v => parseFloat(v)).filter(n => !isNaN(n));
  switch (type) {
    case 'count': return values.length;
    case 'count_values': return values.filter(v => v && v !== '').length;
    case 'count_empty': return values.filter(v => !v || v === '').length;
    case 'sum': return nums.reduce((a, b) => a + b, 0);
    case 'avg': return nums.length ? (nums.reduce((a, b) => a + b, 0) / nums.length).toFixed(1) : '—';
    case 'min': return nums.length ? Math.min(...nums) : '—';
    case 'max': return nums.length ? Math.max(...nums) : '—';
    default: return '';
  }
}

/** Render calculation row HTML */
export function renderCalcRow(fixedColumns, visibleProps, records, valuesMap, idField, tabId) {
  if (!S._dbvCalc) S._dbvCalc = {};
  const calcs = S._dbvCalc[tabId] || {};

  const tdFixed = fixedColumns.map(() => '<td class="calc-cell"></td>').join('');
  const tdProps = visibleProps.map(p => {
    const type = calcs[p.id] || 'none';
    let display = '';
    if (type !== 'none') {
      const values = records.map(r => valuesMap[r[idField]]?.[p.id] ?? '');
      display = `<span class="calc-result">${calcValue(type, values)}</span>`;
    }
    const label = type === 'none' ? 'Calculate' : CALC_TYPES.find(t => t.id === type)?.label || type;
    return `<td class="calc-cell" data-calc-prop="${p.id}"><span class="calc-label">${label}</span>${display}</td>`;
  }).join('');

  return `<tr class="calc-row">${tdFixed}${tdProps}<td class="calc-cell"></td></tr>`;
}

/** Bind click events for calculation cells to show picker */
export function bindCalcEvents(container, tabId, visibleProps, reloadFn) {
  container.querySelectorAll('.calc-cell[data-calc-prop]').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      showCalcPicker(cell, tabId, parseInt(cell.dataset.calcProp), reloadFn);
    });
  });
}

function showCalcPicker(anchor, tabId, propId, reloadFn) {
  document.querySelectorAll('.calc-picker').forEach(m => m.remove());
  const menu = document.createElement('div');
  menu.className = 'col-context-menu calc-picker';
  if (!S._dbvCalc) S._dbvCalc = {};
  const current = S._dbvCalc[tabId]?.[propId] || 'none';
  menu.innerHTML = CALC_TYPES.map(t =>
    `<div class="col-menu-item${t.id === current ? ' active' : ''}" data-calc="${t.id}">${t.label}</div>`
  ).join('');
  const rect = anchor.getBoundingClientRect();
  menu.style.left = rect.left + 'px';
  menu.style.top = (rect.top - CALC_TYPES.length * 30 - 8) + 'px';
  document.body.appendChild(menu);
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', () => {
      if (!S._dbvCalc[tabId]) S._dbvCalc[tabId] = {};
      S._dbvCalc[tabId][propId] = item.dataset.calc;
      menu.remove();
      if (reloadFn) reloadFn();
    });
  });
  setTimeout(() => document.addEventListener('mousedown', (e) => {
    if (!menu.contains(e.target)) menu.remove();
  }, { once: true }), 10);
}
