// ── dev-cefr.js — CEFR level chip (A1–C2) and picker popup ──

import { invoke } from './state.js';

export const CEFR_LEVELS = ['A1', 'A2', 'B1', 'B2', 'C1', 'C2'];

/** Render the level chip; empty/missing level → empty string. */
export function cefrChipHtml(level, nodeId) {
  const lvl = (level || '').trim();
  if (!lvl) return `<button class="dev-cefr-chip dev-cefr-empty" data-cefr-edit="${nodeId}" title="Назначить CEFR-уровень">+</button>`;
  return `<button class="dev-cefr-chip" data-cefr="${lvl}" data-cefr-edit="${nodeId}" title="CEFR-уровень — клик для смены">${lvl}</button>`;
}

/** Popup with 6 levels + clear; calls onPick(newLevel|''). */
export function showCefrPicker(currentLevel, onPick) {
  const overlay = document.createElement('div');
  overlay.className = 'cefr-picker-overlay';
  overlay.innerHTML = `<div class="cefr-picker">
    ${CEFR_LEVELS.map(l => `<button class="cefr-opt${l === currentLevel ? ' active' : ''}" data-cefr="${l}" data-level="${l}">${l}</button>`).join('')}
    <button class="cefr-opt cefr-clear" data-level="" title="Очистить">×</button>
  </div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) { close(); return; }
    const btn = e.target.closest('[data-level]');
    if (!btn) return;
    close();
    onPick(btn.dataset.level);
  });
}

/** Wire all .dev-cefr-chip[data-cefr-edit] in `el` to open the picker. */
export function wireCefrChips(el, reloadFn) {
  el.querySelectorAll('[data-cefr-edit]').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault(); e.stopPropagation();
      const id = parseInt(btn.dataset.cefrEdit);
      const current = btn.dataset.cefr || '';
      showCefrPicker(current, async (level) => {
        await invoke('update_dev_node', {
          id, name: null, score: null, theory: null, material: null, priority: null, level,
        });
        reloadFn();
      });
    });
  });
}
