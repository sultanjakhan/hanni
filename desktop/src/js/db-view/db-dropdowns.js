// ── db-view/db-dropdowns.js — Select & multi-select dropdown UI (Notion-style) ──

import { escapeHtml } from '../utils.js';
import { invoke } from '../state.js';

const BADGE_COLORS = ['blue', 'green', 'yellow', 'red', 'purple', 'orange', 'pink', 'gray'];
const stripEmoji = (s) => s.replace(/[\p{Emoji_Presentation}\p{Extended_Pictographic}]/gu, '').trim();
function badgeColorFor(val, allOpts) {
  const idx = allOpts.indexOf(val);
  return BADGE_COLORS[idx >= 0 ? idx % BADGE_COLORS.length : 0];
}

function repositionDropdown(dd, anchorRect) {
  const list = dd.querySelector('.inline-dd-list');
  if (list) list.style.maxHeight = '';
  const r = dd.getBoundingClientRect();
  const spaceBelow = window.innerHeight - anchorRect.bottom - 8;
  const spaceAbove = anchorRect.top - 8;
  const headerHeight = 70;
  if (r.height > spaceBelow) {
    if (spaceAbove > spaceBelow) {
      const available = Math.min(r.height, spaceAbove);
      dd.style.top = Math.max(4, anchorRect.top - available - 2) + 'px';
      if (list) list.style.maxHeight = (available - headerHeight) + 'px';
    } else {
      if (list) list.style.maxHeight = (spaceBelow - headerHeight) + 'px';
    }
  }
  if (r.right > window.innerWidth) dd.style.left = Math.max(4, window.innerWidth - r.width - 8) + 'px';
}

function makePersist(propId, onOptionsChange) {
  if (onOptionsChange) return onOptionsChange;
  if (propId) return (opts) => invoke('update_property_definition', { id: propId, name: null, propType: null, position: null, color: null, options: JSON.stringify(opts), visible: null }).catch(() => {});
  return null;
}

function closeDropdown(dd, closeRef) {
  if (!dd.parentNode) return;
  dd.remove();
  if (closeRef.fn) { document.removeEventListener('mousedown', closeRef.fn); closeRef.fn = null; }
}

// ── Inline rename for an option ──
function startOptionRename(badgeEl, val, onRename) {
  if (badgeEl.querySelector('.inline-dd-rename')) return;
  const oldText = badgeEl.textContent.trim();
  badgeEl.textContent = '';
  const inp = document.createElement('input');
  inp.className = 'inline-dd-rename';
  inp.value = oldText;
  badgeEl.appendChild(inp);
  inp.focus();
  inp.select();
  let saved = false;
  const save = () => {
    if (saved) return;
    saved = true;
    const newName = inp.value.trim();
    if (newName && newName !== oldText) onRename(val, newName);
    else onRename(null, null);
  };
  const cancel = () => { if (!saved) onRename(null, null); };
  inp.addEventListener('blur', cancel);
  inp.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); save(); inp.blur(); }
    if (e.key === 'Escape') { inp.blur(); }
    e.stopPropagation();
  });
}

// ── Single Select ──

export function showSelectDropdown(cell, options, currentVal, save, propId, onOptionsChange) {
  closeAllDropdowns();
  const rect = cell.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'inline-dropdown';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 2 + 'px';
  dd.style.minWidth = Math.max(rect.width, 200) + 'px';
  let current = currentVal;
  if (current && !options.some(o => o.value === current)) {
    options.push({ value: current, label: current });
  }
  const allVals = () => options.map(o => o.value);
  const persist = makePersist(propId, onOptionsChange);
  const canEdit = !!persist;
  const dopersist = () => { if (persist) persist(allVals()); };
  const closeRef = { fn: null };

  const renderSelected = () => {
    const area = dd.querySelector('.inline-dd-selected');
    if (!current) { area.innerHTML = '<span class="text-faint" style="font-size:12px;">Ничего не выбрано</span>'; return; }
    const label = options.find(o => o.value === current)?.label || current;
    const color = badgeColorFor(current, allVals());
    area.innerHTML = `<span class="badge badge-${color}">${escapeHtml(label)}<span class="inline-dd-tag-x" data-clear="1" title="Убрать"> ✕</span></span>`;
    area.querySelector('[data-clear]')?.addEventListener('click', (e) => {
      e.stopPropagation();
      current = null;
      closeDropdown(dd, closeRef);
      save('');
    });
  };

  const renderOptions = (filter) => {
    const list = dd.querySelector('.inline-dd-list');
    let filtered = filter ? options.filter(o => o.label.toLowerCase().includes(filter.toLowerCase())) : [...options];
    filtered.sort((a, b) => stripEmoji(a.label).localeCompare(stripEmoji(b.label)));
    let html = filtered.map(o => {
      const color = badgeColorFor(o.value, allVals());
      return `<div class="inline-dd-option${o.value === current ? ' active' : ''}" data-val="${escapeHtml(o.value)}">
        <span class="badge badge-${color} inline-dd-badge">${escapeHtml(o.label)}</span>
        ${canEdit ? `<span class="inline-dd-rename-btn" data-rename="${escapeHtml(o.value)}" title="Переименовать">✎</span>` : ''}
        ${canEdit ? `<span class="inline-dd-remove" data-remove="${escapeHtml(o.value)}" title="Удалить из каталога">✕</span>` : ''}
      </div>`;
    }).join('');
    if (filter && !filtered.some(o => o.label.toLowerCase() === filter.toLowerCase())) {
      html += `<div class="inline-dd-option inline-dd-create" data-val="${escapeHtml(filter)}">+ Создать «${escapeHtml(filter)}»</div>`;
    }
    list.innerHTML = html;
    if (canEdit) {
      list.querySelectorAll('.inline-dd-rename-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
          e.stopPropagation();
          const opt = btn.closest('.inline-dd-option');
          const badge = opt?.querySelector('.inline-dd-badge');
          const val = opt?.dataset.val;
          if (!badge || !val) return;
          startOptionRename(badge, val, (oldVal, newName) => {
            if (oldVal && newName) {
              const o = options.find(x => x.value === oldVal);
              if (o) { o.label = newName; o.value = newName; }
              if (current === oldVal) current = newName;
              dopersist();
            }
            renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
          });
        });
      });
    }
    list.querySelectorAll('.inline-dd-remove').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const val = btn.dataset.remove;
        const idx = options.findIndex(o => o.value === val);
        if (idx >= 0) options.splice(idx, 1);
        if (current === val) { current = null; renderSelected(); save(''); }
        dopersist();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
    list.querySelectorAll('.inline-dd-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        if (e.target.closest('.inline-dd-remove') || e.target.closest('.inline-dd-rename') || e.target.closest('.inline-dd-rename-btn')) return;
        const val = opt.dataset.val;
        if (opt.classList.contains('inline-dd-create')) {
          options.push({ value: val, label: val });
          dopersist();
        }
        closeDropdown(dd, closeRef);
        save(val);
      });
    });
  };

  dd.innerHTML = `<div class="inline-dd-selected"></div><div class="inline-dd-search-wrap"><input class="inline-dd-search" placeholder="Поиск или создать..."></div><div class="inline-dd-list"></div>`;
  document.body.appendChild(dd);
  renderSelected();
  renderOptions('');
  repositionDropdown(dd, rect);

  const input = dd.querySelector('.inline-dd-search');
  input.focus();
  input.addEventListener('input', () => renderOptions(input.value.trim()));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const val = input.value.trim();
      if (val) {
        if (!options.some(o => o.value === val)) { options.push({ value: val, label: val }); dopersist(); }
        closeDropdown(dd, closeRef);
        save(val);
      }
    }
    if (e.key === 'Escape') closeDropdown(dd, closeRef);
    e.stopPropagation();
  });

  setTimeout(() => {
    closeRef.fn = (e) => { if (!dd.contains(e.target)) closeDropdown(dd, closeRef); };
    document.addEventListener('mousedown', closeRef.fn);
  }, 10);
}

// ── Multi Select ──

export function showMultiSelectDropdown(cell, options, rawVal, save, propId, labelMap, onOptionsChange) {
  closeAllDropdowns();
  let selected = [];
  try { selected = JSON.parse(rawVal || '[]'); } catch { selected = rawVal ? [rawVal] : []; }
  let allOptions = [...options];
  selected.forEach(v => { if (v && !allOptions.includes(v)) allOptions.push(v); });
  const labels = labelMap || {};
  const label = (v) => labels[v] || v;
  const persist = makePersist(propId, onOptionsChange);
  const canEdit = !!persist;
  const dopersist = () => { if (persist) persist([...allOptions]); };
  const closeRef = { fn: null };

  const doSave = () => { save(selected.length > 0 ? JSON.stringify(selected) : ''); };

  const rect = cell.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'inline-dropdown inline-dd-multi';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 2 + 'px';
  dd.style.minWidth = Math.max(rect.width, 200) + 'px';

  const renderSelected = () => {
    const area = dd.querySelector('.inline-dd-selected');
    if (selected.length === 0) { area.innerHTML = '<span class="text-faint" style="font-size:12px;">Ничего не выбрано</span>'; return; }
    area.innerHTML = selected.map(v => {
      const color = badgeColorFor(v, allOptions);
      return `<span class="badge badge-${color}">${escapeHtml(label(v))}<span class="inline-dd-tag-x" data-untag="${escapeHtml(v)}"> ✕</span></span>`;
    }).join(' ');
    area.querySelectorAll('[data-untag]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        selected = selected.filter(x => x !== btn.dataset.untag);
        doSave();
        renderSelected();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
  };

  const renderOptions = (filter) => {
    const list = dd.querySelector('.inline-dd-list');
    let filtered = filter ? allOptions.filter(o => label(o).toLowerCase().includes(filter.toLowerCase())) : [...allOptions];
    filtered.sort((a, b) => stripEmoji(label(a)).localeCompare(stripEmoji(label(b))));
    let html = filtered.map(o => {
      const color = badgeColorFor(o, allOptions);
      return `<div class="inline-dd-option${selected.includes(o) ? ' active' : ''}" data-val="${escapeHtml(o)}">
        <span class="inline-dd-check">${selected.includes(o) ? '\u2713' : ''}</span>
        <span class="badge badge-${color} inline-dd-badge">${escapeHtml(label(o))}</span>
        ${canEdit ? `<span class="inline-dd-rename-btn" data-rename="${escapeHtml(o)}" title="Переименовать">✎</span>` : ''}
        ${canEdit ? `<span class="inline-dd-remove" data-remove="${escapeHtml(o)}" title="Удалить из каталога">✕</span>` : ''}
      </div>`;
    }).join('');
    if (filter && !allOptions.some(o => label(o).toLowerCase() === filter.toLowerCase())) {
      html += `<div class="inline-dd-option inline-dd-create" data-val="${escapeHtml(filter)}">+ Создать «${escapeHtml(filter)}»</div>`;
    }
    list.innerHTML = html;
    if (canEdit) {
      list.querySelectorAll('.inline-dd-rename-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
          e.stopPropagation();
          const opt = btn.closest('.inline-dd-option');
          const badge = opt?.querySelector('.inline-dd-badge');
          const val = opt?.dataset.val;
          if (!badge || !val) return;
          startOptionRename(badge, val, (oldVal, newName) => {
            if (oldVal && newName) {
              const idx = allOptions.indexOf(oldVal);
              if (idx >= 0) allOptions[idx] = newName;
              labels[newName] = newName;
              if (labels[oldVal]) delete labels[oldVal];
              selected = selected.map(s => s === oldVal ? newName : s);
              dopersist();
              doSave();
            }
            renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
            renderSelected();
          });
        });
      });
    }
    list.querySelectorAll('.inline-dd-remove').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const val = btn.dataset.remove;
        allOptions = allOptions.filter(o => o !== val);
        selected = selected.filter(x => x !== val);
        delete labels[val];
        dopersist();
        doSave();
        renderSelected();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
    list.querySelectorAll('.inline-dd-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        if (e.target.closest('.inline-dd-remove') || e.target.closest('.inline-dd-rename') || e.target.closest('.inline-dd-rename-btn')) return;
        e.stopPropagation();
        const v = opt.dataset.val;
        if (opt.classList.contains('inline-dd-create')) {
          allOptions.push(v);
          selected.push(v);
          dopersist();
        } else if (selected.includes(v)) {
          selected = selected.filter(x => x !== v);
        } else {
          selected.push(v);
        }
        doSave();
        renderSelected();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
  };

  dd.innerHTML = `<div class="inline-dd-selected"></div><div class="inline-dd-search-wrap"><input class="inline-dd-search" placeholder="Поиск или создать..."></div><div class="inline-dd-list"></div>`;
  document.body.appendChild(dd);
  renderSelected();
  renderOptions('');
  repositionDropdown(dd, rect);

  const input = dd.querySelector('.inline-dd-search');
  input.focus();
  input.addEventListener('input', () => renderOptions(input.value.trim()));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeDropdown(dd, closeRef);
    e.stopPropagation();
  });

  setTimeout(() => {
    closeRef.fn = (e) => { if (!dd.contains(e.target)) closeDropdown(dd, closeRef); };
    document.addEventListener('mousedown', closeRef.fn);
  }, 10);
}

export function closeAllDropdowns() {
  document.querySelectorAll('.inline-dropdown').forEach(d => d.remove());
}
