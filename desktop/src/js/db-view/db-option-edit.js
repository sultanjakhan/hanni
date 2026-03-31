// ── db-view/db-option-edit.js — Option edit panel (Notion-style sidebar) ──

import { escapeHtml } from '../utils.js';
import { BADGE_COLORS } from './db-dropdowns.js';

export function showOptionEditPanel(anchor, opt, allOptions, dopersist, refreshFn) {
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
