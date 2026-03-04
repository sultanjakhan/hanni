// ── db-view/db-filters.js — Filter builder, persistence ──

import { S, invoke } from '../state.js';
import { escapeHtml } from '../utils.js';

/** Render filter chip bar above the database view */
export function renderFilterBar(el, tabId, customProps, onApply) {
  const filters = S.dbvFilters[tabId] || [];
  const chips = filters.map((f, idx) => {
    const prop = customProps.find(p => p.id === f.propId);
    const label = prop ? prop.name : '?';
    const condLabels = { eq: '=', neq: '\u2260', contains: '\u2248', empty: 'empty', not_empty: 'not empty' };
    return `<span class="dbv-filter-chip" data-idx="${idx}">
      ${escapeHtml(label)} ${condLabels[f.condition] || f.condition} ${f.value ? escapeHtml(f.value) : ''}
      <span class="dbv-filter-chip-remove" data-remove="${idx}">\u00d7</span>
    </span>`;
  }).join('');

  const bar = document.createElement('div');
  bar.className = 'dbv-filter-bar';
  bar.innerHTML = `<button class="btn-secondary dbv-add-filter-btn">+ Filter</button>${chips}`;
  el.prepend(bar);

  bar.querySelector('.dbv-add-filter-btn')?.addEventListener('click', () => {
    showFilterBuilderModal(tabId, customProps, onApply);
  });

  bar.querySelectorAll('.dbv-filter-chip-remove').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const idx = parseInt(btn.dataset.remove);
      if (S.dbvFilters[tabId]) S.dbvFilters[tabId].splice(idx, 1);
      saveFiltersToViewConfig(tabId);
      onApply();
    });
  });
}

/** Show the filter builder modal */
export function showFilterBuilderModal(tabId, customProps, onApply) {
  if (customProps.length === 0) { alert('Add custom properties first to filter by them.'); return; }
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add Filter</div>
    <div class="form-group"><label class="form-label">Property</label>
      <select class="form-select" id="dbv-filter-prop" style="width:100%;">
        ${customProps.map(p => `<option value="${p.id}" data-type="${p.type}" data-options='${escapeHtml(p.options || "[]")}'>${escapeHtml(p.name)}</option>`).join('')}
      </select>
    </div>
    <div class="form-group"><label class="form-label">Condition</label>
      <select class="form-select" id="dbv-filter-cond" style="width:100%;">
        <option value="eq">Equals</option><option value="neq">Not equals</option>
        <option value="contains">Contains</option>
        <option value="empty">Is empty</option><option value="not_empty">Is not empty</option>
      </select>
    </div>
    <div class="form-group" id="dbv-filter-val-group"><label class="form-label">Value</label>
      <input class="form-input" id="dbv-filter-val" placeholder="Value">
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="dbv-filter-apply">Apply</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  const updateValueInput = () => {
    const sel = document.getElementById('dbv-filter-prop');
    const opt = sel?.selectedOptions[0];
    const type = opt?.dataset.type;
    const cond = document.getElementById('dbv-filter-cond')?.value;
    const valGroup = document.getElementById('dbv-filter-val-group');

    if (cond === 'empty' || cond === 'not_empty') {
      valGroup.style.display = 'none';
      return;
    }
    valGroup.style.display = 'block';

    if (type === 'select' || type === 'multi_select') {
      let options = [];
      try { options = JSON.parse(opt?.dataset.options || '[]'); } catch {}
      valGroup.innerHTML = `<label class="form-label">Value</label>
        <select class="form-select" id="dbv-filter-val" style="width:100%;">
          ${options.map(o => `<option value="${escapeHtml(o)}">${escapeHtml(o)}</option>`).join('')}
        </select>`;
    } else {
      valGroup.innerHTML = `<label class="form-label">Value</label><input class="form-input" id="dbv-filter-val" placeholder="Value">`;
    }
  };

  document.getElementById('dbv-filter-prop')?.addEventListener('change', updateValueInput);
  document.getElementById('dbv-filter-cond')?.addEventListener('change', updateValueInput);

  document.getElementById('dbv-filter-apply')?.addEventListener('click', () => {
    const propId = parseInt(document.getElementById('dbv-filter-prop')?.value);
    const condition = document.getElementById('dbv-filter-cond')?.value || 'eq';
    const value = document.getElementById('dbv-filter-val')?.value || '';
    if (!S.dbvFilters[tabId]) S.dbvFilters[tabId] = [];
    S.dbvFilters[tabId].push({ propId, condition, value });
    overlay.remove();
    saveFiltersToViewConfig(tabId);
    onApply();
  });
}

/** Apply filters to records based on property values */
export function applyFilters(records, valuesMap, filters, idField) {
  if (!filters || filters.length === 0) return records;
  return records.filter(r => {
    const rid = r[idField];
    return filters.every(f => {
      const val = valuesMap[rid]?.[f.propId] ?? '';
      switch (f.condition) {
        case 'eq': return val === f.value;
        case 'neq': return val !== f.value;
        case 'contains': return String(val).toLowerCase().includes(f.value.toLowerCase());
        case 'empty': return !val;
        case 'not_empty': return !!val;
        default: return true;
      }
    });
  });
}

/** Persist filters to view_configs table */
export async function saveFiltersToViewConfig(tabId) {
  const filters = S.dbvFilters[tabId] || [];
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0) {
      await invoke('update_view_config', { id: configs[0].id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    } else {
      const id = await invoke('create_view_config', { tabId, name: 'Default', viewType: 'table' });
      await invoke('update_view_config', { id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    }
  } catch {}
}

/** Load filters from view_configs table */
export async function loadFiltersFromViewConfig(tabId) {
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0 && configs[0].filter_json) {
      S.dbvFilters[tabId] = JSON.parse(configs[0].filter_json);
    }
  } catch {}
}
