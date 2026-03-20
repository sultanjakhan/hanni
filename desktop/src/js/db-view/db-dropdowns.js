// ── db-view/db-dropdowns.js — Select & multi-select dropdown UI ──

import { escapeHtml } from '../utils.js';

export function showSelectDropdown(cell, options, currentVal, save) {
  closeAllDropdowns();
  const rect = cell.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'inline-dropdown';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 2 + 'px';
  dd.style.minWidth = Math.max(rect.width, 160) + 'px';

  const renderOptions = (filter) => {
    const list = dd.querySelector('.inline-dd-list');
    const filtered = filter
      ? options.filter(o => o.label.toLowerCase().includes(filter.toLowerCase()))
      : options;
    let html = filtered.map(o =>
      `<div class="inline-dd-option${o.value === currentVal ? ' active' : ''}" data-val="${escapeHtml(o.value)}">${escapeHtml(o.label)}</div>`
    ).join('');
    if (filter && !filtered.some(o => o.label.toLowerCase() === filter.toLowerCase())) {
      html += `<div class="inline-dd-option inline-dd-create" data-val="${escapeHtml(filter)}">+ ${escapeHtml(filter)}</div>`;
    }
    html += `<div class="inline-dd-option inline-dd-clear" data-val="">\u2014 Очистить</div>`;
    list.innerHTML = html;
    list.querySelectorAll('.inline-dd-option').forEach(opt => {
      opt.addEventListener('click', () => { dd.remove(); save(opt.dataset.val || null); });
    });
  };

  dd.innerHTML = `<div class="inline-dd-search-wrap"><input class="inline-dd-search" placeholder="Поиск или создать..."></div><div class="inline-dd-list"></div>`;
  document.body.appendChild(dd);
  renderOptions('');

  const input = dd.querySelector('.inline-dd-search');
  input.focus();
  input.addEventListener('input', () => renderOptions(input.value.trim()));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); const val = input.value.trim(); if (val) { dd.remove(); save(val); } }
    if (e.key === 'Escape') dd.remove();
    e.stopPropagation();
  });

  setTimeout(() => {
    const close = (e) => { if (!dd.contains(e.target)) { dd.remove(); document.removeEventListener('mousedown', close); } };
    document.addEventListener('mousedown', close);
  }, 10);
}

export function showMultiSelectDropdown(cell, options, rawVal, save) {
  closeAllDropdowns();
  let selected = [];
  try { selected = JSON.parse(rawVal || '[]'); } catch { selected = rawVal ? [rawVal] : []; }

  const rect = cell.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'inline-dropdown inline-dd-multi';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 2 + 'px';
  dd.style.minWidth = Math.max(rect.width, 160) + 'px';

  const render = () => {
    dd.innerHTML = options.map(o =>
      `<div class="inline-dd-option${selected.includes(o) ? ' active' : ''}" data-val="${escapeHtml(o)}">
        <span class="inline-dd-check">${selected.includes(o) ? '\u2713' : ''}</span>${escapeHtml(o)}
      </div>`
    ).join('');
    dd.querySelectorAll('.inline-dd-option').forEach(opt => {
      opt.addEventListener('click', (e) => {
        e.stopPropagation();
        const v = opt.dataset.val;
        if (selected.includes(v)) selected = selected.filter(x => x !== v);
        else selected.push(v);
        render();
      });
    });
  };
  render();
  document.body.appendChild(dd);

  setTimeout(() => {
    const close = (e) => {
      if (!dd.contains(e.target)) {
        dd.remove();
        document.removeEventListener('mousedown', close);
        save(selected.length > 0 ? JSON.stringify(selected) : null);
      }
    };
    document.addEventListener('mousedown', close);
  }, 10);
}

export function closeAllDropdowns() {
  document.querySelectorAll('.inline-dropdown').forEach(d => d.remove());
}
