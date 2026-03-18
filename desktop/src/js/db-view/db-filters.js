import { S, invoke } from '../state.js';
import { escapeHtml } from '../utils.js';

export function renderFilterBar(el, tabId, customProps, onApply) {
  const filters = S.dbvFilters[tabId] || [];
  const chips = filters.map((f, idx) => {
    const prop = customProps.find(p => p.id === f.propId);
    const label = prop ? prop.name : '?';
    const condLabels = { eq: '=', neq: '\u2260', contains: '\u2248', starts: 'starts', ends: 'ends', before: '<', after: '>', this_week: 'this week', this_month: 'this month', empty: 'empty', not_empty: 'not empty' };
    return `<span class="dbv-filter-chip" data-idx="${idx}">
      ${escapeHtml(label)} ${condLabels[f.condition] || f.condition} ${f.value ? escapeHtml(f.value) : ''}
      <span class="dbv-filter-chip-remove" data-remove="${idx}">\u00d7</span>
    </span>`;
  }).join('');

  const mode = S.dbvFilterMode?.[tabId] || 'and';
  const modeBtn = filters.length > 1
    ? `<button class="btn-secondary dbv-filter-mode-btn">${mode === 'and' ? 'AND' : 'OR'}</button>` : '';

  const bar = document.createElement('div');
  bar.className = 'dbv-filter-bar';
  bar.innerHTML = `<button class="btn-secondary dbv-add-filter-btn">+ Filter</button>${modeBtn}${chips}`;
  el.prepend(bar);

  bar.querySelector('.dbv-add-filter-btn')?.addEventListener('click', () => {
    showFilterBuilderModal(tabId, customProps, onApply);
  });

  bar.querySelector('.dbv-filter-mode-btn')?.addEventListener('click', () => {
    if (!S.dbvFilterMode) S.dbvFilterMode = {};
    S.dbvFilterMode[tabId] = mode === 'and' ? 'or' : 'and';
    onApply();
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

export function showFilterBuilderModal(tabId, customProps, onApply) {
  if (!customProps.length) { alert('Сначала добавьте свойства для фильтрации.'); return; }
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
        <option value="starts">Starts with</option><option value="ends">Ends with</option>
        <option value="before">Before (date)</option><option value="after">After (date)</option>
        <option value="this_week">This week</option><option value="this_month">This month</option>
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
    const opt = document.getElementById('dbv-filter-prop')?.selectedOptions[0];
    const cond = document.getElementById('dbv-filter-cond')?.value;
    const vg = document.getElementById('dbv-filter-val-group');
    if (['empty', 'not_empty', 'this_week', 'this_month'].includes(cond)) { vg.style.display = 'none'; return; }
    vg.style.display = 'block';
    const type = opt?.dataset.type;
    if (type === 'select' || type === 'multi_select' || type === 'status') {
      let opts = []; try { opts = JSON.parse(opt?.dataset.options || '[]'); } catch {}
      if (type === 'status' && opts.length === 0) opts = ['Не начато', 'В работе', 'Готово'];
      vg.innerHTML = `<label class="form-label">Value</label><select class="form-select" id="dbv-filter-val" style="width:100%;">${opts.map(o => `<option value="${escapeHtml(o)}">${escapeHtml(o)}</option>`).join('')}</select>`;
    } else {
      vg.innerHTML = `<label class="form-label">Value</label><input class="form-input" id="dbv-filter-val" placeholder="Value">`;
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

export function applyFilters(records, valuesMap, filters, idField, tabId) {
  if (!filters || filters.length === 0) return records;
  const mode = S.dbvFilterMode?.[tabId] || 'and';
  const check = (f, rid) => {
    const val = valuesMap[rid]?.[f.propId] ?? '';
    const s = String(val).toLowerCase(), fv = (f.value || '').toLowerCase();
    switch (f.condition) {
      case 'eq': return val === f.value;
      case 'neq': return val !== f.value;
      case 'contains': return s.includes(fv);
      case 'starts': return s.startsWith(fv);
      case 'ends': return s.endsWith(fv);
      case 'before': return val && val < f.value;
      case 'after': return val && val > f.value;
      case 'this_week': { const d = new Date(), w = new Date(d); w.setDate(d.getDate() - d.getDay()); return val >= w.toISOString().slice(0, 10); }
      case 'this_month': { const d = new Date(); return val >= `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-01`; }
      case 'empty': return !val;
      case 'not_empty': return !!val;
      default: return true;
    }
  };
  return records.filter(r => {
    const rid = r[idField];
    return mode === 'or' ? filters.some(f => check(f, rid)) : filters.every(f => check(f, rid));
  });
}

export async function saveFiltersToViewConfig(tabId) {
  const json = JSON.stringify(S.dbvFilters[tabId] || []);
  try {
    const c = await invoke('get_view_configs', { tabId });
    const id = c[0]?.id || await invoke('create_view_config', { tabId, name: 'Default', viewType: 'table' });
    await invoke('update_view_config', { id, filterJson: json, sortJson: null, visibleColumns: null });
  } catch {}
}

export async function loadFiltersFromViewConfig(tabId) {
  try {
    const c = await invoke('get_view_configs', { tabId });
    if (c[0]?.filter_json) S.dbvFilters[tabId] = JSON.parse(c[0].filter_json);
  } catch {}
}
