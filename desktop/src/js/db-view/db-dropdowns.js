// ── db-view/db-dropdowns.js — Unified select dropdown (Notion-style) ──

import { escapeHtml } from '../utils.js';
import { invoke } from '../state.js';

export const BADGE_COLORS = ['blue', 'green', 'yellow', 'red', 'purple', 'orange', 'pink', 'gray'];
const stripEmoji = (s) => s.replace(/[\p{Emoji_Presentation}\p{Extended_Pictographic}]/gu, '').trim();

/** Normalize options: strings or {value,color} → [{value, color}] */
export function normalizeOptions(opts) {
  if (!opts || !Array.isArray(opts)) return [];
  return opts.map((o, i) => {
    if (typeof o === 'object' && o.value != null) return { value: o.value, color: o.color || BADGE_COLORS[i % BADGE_COLORS.length] };
    const val = typeof o === 'object' ? (o.label || String(o)) : String(o);
    return { value: val, color: BADGE_COLORS[i % BADGE_COLORS.length] };
  });
}

export function colorForValue(val, options) {
  return (options.find(o => o.value === val))?.color || BADGE_COLORS[0];
}

export function serializeOptions(opts) {
  return JSON.stringify(opts.map(o => ({ value: o.value, color: o.color })));
}

function repositionDropdown(dd, anchorRect) {
  const list = dd.querySelector('.inline-dd-list');
  if (list) list.style.maxHeight = '';
  const r = dd.getBoundingClientRect();
  const spaceBelow = window.innerHeight - anchorRect.bottom - 8;
  const spaceAbove = anchorRect.top - 8;
  const hdr = 90;
  if (r.height > spaceBelow) {
    if (spaceAbove > spaceBelow) {
      const avail = Math.min(r.height, spaceAbove);
      dd.style.top = Math.max(4, anchorRect.top - avail - 2) + 'px';
      if (list) list.style.maxHeight = (avail - hdr) + 'px';
    } else {
      if (list) list.style.maxHeight = (spaceBelow - hdr) + 'px';
    }
  }
  if (r.right > window.innerWidth) dd.style.left = Math.max(4, window.innerWidth - r.width - 8) + 'px';
}

function makePersist(propId, onOptionsChange) {
  if (onOptionsChange) return onOptionsChange;
  if (propId) return (opts) => invoke('update_property_definition', {
    id: propId, name: null, propType: null, position: null, color: null,
    options: serializeOptions(opts), visible: null,
  }).catch(() => {});
  return null;
}

function closeDropdown(dd, closeRef) {
  if (!dd.parentNode) return;
  dd.remove();
  if (closeRef.fn) { document.removeEventListener('mousedown', closeRef.fn); closeRef.fn = null; }
}

// ── Option edit panel (Notion-style) ──
function showOptionEditPanel(anchor, opt, allOptions, dopersist, refreshFn) {
  document.querySelectorAll('.opt-edit-panel').forEach(p => p.remove());
  const panel = document.createElement('div');
  panel.className = 'opt-edit-panel';

  const colorsHtml = BADGE_COLORS.map(c =>
    `<div class="opt-edit-color-row${c === opt.color ? ' active' : ''}" data-color="${c}">` +
    `<span class="opt-edit-color-swatch badge-${c}"></span>` +
    `<span>${c.charAt(0).toUpperCase() + c.slice(1)}</span>` +
    (c === opt.color ? '<span class="opt-edit-check">\u2713</span>' : '') +
    `</div>`
  ).join('');

  panel.innerHTML =
    `<div class="opt-edit-section"><input class="opt-edit-rename" value="${escapeHtml(opt.value)}" /></div>` +
    `<div class="opt-edit-divider"></div>` +
    `<div class="opt-edit-section"><div class="opt-edit-action danger" data-action="delete"><svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M2 4h12M5.3 4V2.7a1 1 0 011-1h3.4a1 1 0 011 1V4M6.5 7v4.5M9.5 7v4.5M3.5 4l.7 9a1.5 1.5 0 001.5 1.4h4.6a1.5 1.5 0 001.5-1.4l.7-9"/></svg> Delete</div></div>` +
    `<div class="opt-edit-divider"></div>` +
    `<div class="opt-edit-section-label">Colors</div>` +
    `<div class="opt-edit-section opt-edit-colors">${colorsHtml}</div>`;

  const rect = anchor.getBoundingClientRect();
  panel.style.left = (rect.right + 4) + 'px';
  panel.style.top = rect.top + 'px';
  document.body.appendChild(panel);
  if (panel.getBoundingClientRect().right > window.innerWidth - 8) {
    panel.style.left = (rect.left - panel.offsetWidth - 4) + 'px';
  }

  const renameInput = panel.querySelector('.opt-edit-rename');
  renameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const newName = renameInput.value.trim();
      if (newName && newName !== opt.value) { opt.value = newName; dopersist(); }
      panel.remove(); refreshFn();
    }
    if (e.key === 'Escape') { panel.remove(); }
    e.stopPropagation();
  });

  panel.querySelectorAll('.opt-edit-color-row').forEach(row => {
    row.addEventListener('click', (e) => {
      e.stopPropagation();
      opt.color = row.dataset.color;
      dopersist(); panel.remove(); refreshFn();
    });
  });

  panel.querySelector('[data-action="delete"]').addEventListener('click', (e) => {
    e.stopPropagation();
    const idx = allOptions.indexOf(opt);
    if (idx >= 0) allOptions.splice(idx, 1);
    dopersist(); panel.remove(); refreshFn();
  });

  setTimeout(() => {
    const handler = (e) => {
      if (!panel.contains(e.target)) { panel.remove(); document.removeEventListener('mousedown', handler); }
    };
    document.addEventListener('mousedown', handler);
  }, 10);
}

// ── Unified Select Dropdown (Notion-style) ──

export function showSelectDropdown(cell, options, rawVal, save, propId, labelMap, onOptionsChange) {
  closeAllDropdowns();
  let allOptions = normalizeOptions(options);
  let selected = [];
  try { selected = JSON.parse(rawVal || '[]'); } catch { selected = rawVal ? [rawVal] : []; }
  selected.forEach(v => {
    if (v && !allOptions.some(o => o.value === v)) allOptions.push({ value: v, color: BADGE_COLORS[allOptions.length % BADGE_COLORS.length] });
  });
  const labels = labelMap || {};
  const label = (v) => labels[v] || v;

  const persist = makePersist(propId, onOptionsChange);
  const canEdit = !!persist;
  const dopersist = () => { if (persist) persist([...allOptions]); };
  const closeRef = { fn: null };
  const doSave = () => { save(selected.length > 0 ? JSON.stringify(selected) : ''); };

  const rect = cell.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'inline-dropdown';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 2 + 'px';
  dd.style.minWidth = Math.max(rect.width, 220) + 'px';

  const renderSelected = () => {
    const area = dd.querySelector('.inline-dd-selected');
    if (selected.length === 0) { area.innerHTML = ''; area.style.display = 'none'; return; }
    area.style.display = 'flex';
    area.innerHTML = selected.map(v => {
      const color = colorForValue(v, allOptions);
      return `<span class="badge badge-${color}">${escapeHtml(label(v))}<span class="inline-dd-tag-x" data-untag="${escapeHtml(v)}">\u00D7</span></span>`;
    }).join('');
    area.querySelectorAll('[data-untag]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        selected = selected.filter(x => x !== btn.dataset.untag);
        doSave(); renderSelected();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
  };

  const renderOptions = (filter) => {
    const list = dd.querySelector('.inline-dd-list');
    const hint = dd.querySelector('.inline-dd-hint');
    let filtered = filter ? allOptions.filter(o => label(o.value).toLowerCase().includes(filter.toLowerCase())) : [...allOptions];
    filtered.sort((a, b) => stripEmoji(label(a.value)).localeCompare(stripEmoji(label(b.value))));

    if (hint) hint.style.display = (filter || filtered.length === 0) ? 'none' : '';

    let html = filtered.map(o => {
      const isSel = selected.includes(o.value);
      return `<div class="inline-dd-option${isSel ? ' active' : ''}" data-val="${escapeHtml(o.value)}">` +
        (canEdit ? '<span class="inline-dd-drag">\u2237</span>' : '') +
        `<span class="badge badge-${o.color}">${escapeHtml(label(o.value))}</span>` +
        (canEdit ? `<span class="inline-dd-more" data-more="${escapeHtml(o.value)}">\u22EF</span>` : '') +
        `</div>`;
    }).join('');

    if (filter && !allOptions.some(o => label(o.value).toLowerCase() === filter.toLowerCase())) {
      const previewColor = BADGE_COLORS[allOptions.length % BADGE_COLORS.length];
      html += `<div class="inline-dd-option inline-dd-create" data-val="${escapeHtml(filter)}">Create <span class="badge badge-${previewColor}">${escapeHtml(filter)}</span></div>`;
    }
    list.innerHTML = html;

    if (canEdit) {
      list.querySelectorAll('.inline-dd-more').forEach(btn => {
        btn.addEventListener('click', (e) => {
          e.stopPropagation();
          const val = btn.dataset.more;
          const opt = allOptions.find(o => o.value === val);
          if (!opt) return;
          const oldVal = opt.value;
          showOptionEditPanel(btn, opt, allOptions, dopersist, () => {
            if (opt.value !== oldVal) {
              selected = selected.map(s => s === oldVal ? opt.value : s);
              labels[opt.value] = opt.value;
              if (labels[oldVal]) delete labels[oldVal];
            }
            if (!allOptions.some(o => o.value === oldVal)) {
              selected = selected.filter(x => x !== oldVal);
            }
            doSave(); renderSelected();
            renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
          });
        });
      });
    }

    list.querySelectorAll('.inline-dd-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        if (e.target.closest('.inline-dd-more') || e.target.closest('.inline-dd-drag')) return;
        e.stopPropagation();
        const v = opt.dataset.val;
        if (opt.classList.contains('inline-dd-create')) {
          allOptions.push({ value: v, color: BADGE_COLORS[allOptions.length % BADGE_COLORS.length] });
          selected.push(v);
          dopersist();
        } else if (selected.includes(v)) {
          selected = selected.filter(x => x !== v);
        } else {
          selected.push(v);
        }
        doSave(); renderSelected();
        renderOptions(dd.querySelector('.inline-dd-search')?.value.trim() || '');
      });
    });
  };

  dd.innerHTML =
    `<div class="inline-dd-selected"></div>` +
    `<div class="inline-dd-search-wrap"><input class="inline-dd-search" placeholder="" /></div>` +
    `<div class="inline-dd-hint">Select an option or create one</div>` +
    `<div class="inline-dd-list"></div>`;
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
    closeRef.fn = (e) => {
      if (!dd.contains(e.target) && !e.target.closest('.opt-edit-panel')) closeDropdown(dd, closeRef);
    };
    document.addEventListener('mousedown', closeRef.fn);
  }, 10);
}

/** @deprecated alias */
export function showMultiSelectDropdown(cell, options, rawVal, save, propId, labelMap, onOptionsChange) {
  showSelectDropdown(cell, options, rawVal, save, propId, labelMap, onOptionsChange);
}

export function closeAllDropdowns() {
  document.querySelectorAll('.inline-dropdown').forEach(d => d.remove());
  document.querySelectorAll('.opt-edit-panel').forEach(p => p.remove());
}
