// ── db-view/db-sort.js — Multi-level sort dropdown ──

import { S } from '../state.js';
import { escapeHtml } from '../utils.js';

const DIR_LABELS = { asc: 'А → Я', desc: 'Я → А' };

/** Get current sort rules for a tab */
export function getSortRules(tabId) {
  if (!S._dbvSortRules) S._dbvSortRules = {};
  return S._dbvSortRules[tabId] || [];
}

/** Apply multi-level sort to records */
export function applySortRules(records, rules, idField, valuesMap) {
  if (!rules || rules.length === 0) return;
  records.sort((a, b) => {
    for (const { key, dir } of rules) {
      const isProp = key.startsWith('prop_');
      const pid = isProp ? parseInt(key.substring(5)) : null;
      const va = isProp ? (valuesMap[a[idField]]?.[pid] ?? '') : (a[key] ?? '');
      const vb = isProp ? (valuesMap[b[idField]]?.[pid] ?? '') : (b[key] ?? '');
      // Multi-select JSON arrays: sort by element count
      const isJsonA = typeof va === 'string' && va.startsWith('[');
      const isJsonB = typeof vb === 'string' && vb.startsWith('[');
      if (isJsonA || isJsonB) {
        let la = 0, lb = 0;
        try { la = JSON.parse(va || '[]').length; } catch {}
        try { lb = JSON.parse(vb || '[]').length; } catch {}
        const cmp = la - lb;
        if (cmp !== 0) return dir === 'asc' ? cmp : -cmp;
        continue;
      }
      const na = parseFloat(va), nb = parseFloat(vb);
      const cmp = (!isNaN(na) && !isNaN(nb)) ? na - nb : String(va).localeCompare(String(vb));
      if (cmp !== 0) return dir === 'asc' ? cmp : -cmp;
    }
    return 0;
  });
}

/** Show multi-level sort dropdown */
export function showSortDropdown(anchorEl, tabId, customProps, fixedColumns, onApply) {
  document.querySelectorAll('.dbv-sort-dropdown,.dbv-picker-menu').forEach(d => d.remove());
  const allFields = [
    ...fixedColumns.map(c => ({ value: c.key, label: c.label })),
    ...customProps.filter(p => p.visible !== false).map(p => ({ value: `prop_${p.id}`, label: p.name })),
  ];
  if (!S._dbvSortRules) S._dbvSortRules = {};
  const rules = S._dbvSortRules[tabId] || [];

  const rect = anchorEl.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'dbv-sort-dropdown';
  dd.style.top = rect.bottom + 4 + 'px';
  dd.style.right = (window.innerWidth - rect.right) + 'px';
  dd.style.minWidth = '260px';

  const render = () => {
    const rulesHtml = rules.map((r, i) => {
      const f = allFields.find(f => f.value === r.key);
      return `<div class="dbv-sort-rule" data-idx="${i}">
        <span class="dbv-sort-rule-label">${f ? escapeHtml(f.label) : '?'}</span>
        <span class="dbv-sort-rule-dir">${DIR_LABELS[r.dir]}</span>
        <span class="dbv-sort-rule-remove" data-idx="${i}">×</span>
      </div>`;
    }).join('');

    dd.innerHTML = `
      ${rulesHtml || '<div style="font-size:12px;color:var(--text-faint);padding:4px 0;">Нет сортировок</div>'}
      <div style="border-top:1px solid var(--border-subtle);margin:6px 0;"></div>
      <div class="dbv-fd-row"><div class="dbv-fd-picker" id="dbv-sd-field">${escapeHtml(allFields[0]?.label || '—')}<span class="dbv-fd-arrow">▾</span></div></div>
      <div class="dbv-fd-row"><div class="dbv-fd-picker" id="dbv-sd-dir">А → Я<span class="dbv-fd-arrow">▾</span></div></div>
      <div style="display:flex;gap:6px;">
        <button class="dbv-fd-apply" style="flex:1;">+ Добавить</button>
        ${rules.length > 0 ? '<button class="dbv-fd-apply dbv-sort-clear" style="flex:1;color:var(--color-red);">Сбросить</button>' : ''}
      </div>`;

    let selField = allFields[0], selDir = 'asc';

    dd.querySelector('#dbv-sd-field')?.addEventListener('click', () => {
      showPicker(dd.querySelector('#dbv-sd-field'), allFields, selField?.value, (v) => { selField = allFields.find(f => f.value === v); dd.querySelector('#dbv-sd-field').firstChild.textContent = selField?.label || ''; });
    });
    dd.querySelector('#dbv-sd-dir')?.addEventListener('click', () => {
      showPicker(dd.querySelector('#dbv-sd-dir'), [{ value: 'asc', label: 'А → Я' }, { value: 'desc', label: 'Я → А' }], selDir, (v) => { selDir = v; dd.querySelector('#dbv-sd-dir').firstChild.textContent = DIR_LABELS[v]; });
    });
    dd.querySelector('.dbv-fd-apply:not(.dbv-sort-clear)')?.addEventListener('click', () => {
      if (selField) { rules.push({ key: selField.value, dir: selDir }); S._dbvSortRules[tabId] = rules; applyAndRerender(); }
    });
    dd.querySelector('.dbv-sort-clear')?.addEventListener('click', () => {
      S._dbvSortRules[tabId] = []; dd.remove(); if (onApply) onApply();
    });
    dd.querySelectorAll('.dbv-sort-rule-remove').forEach(btn => {
      btn.addEventListener('click', (e) => { e.stopPropagation(); rules.splice(parseInt(btn.dataset.idx), 1); S._dbvSortRules[tabId] = rules; applyAndRerender(); });
    });
  };

  const applyAndRerender = () => { render(); if (onApply) onApply(); };
  render();
  document.body.appendChild(dd);
  setTimeout(() => { const close = (e) => { if (!dd.contains(e.target) && !anchorEl.contains(e.target) && !e.target.closest('.dbv-picker-menu')) { dd.remove(); document.removeEventListener('mousedown', close); } }; document.addEventListener('mousedown', close); }, 10);
}

function showPicker(anchor, options, currentVal, onSelect) {
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
