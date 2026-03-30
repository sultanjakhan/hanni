// jobs-edit.js — Shared edit modal for Jobs Memory tabs
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

/**
 * Show edit modal for a facts entry.
 * @param {string} category — facts category (jobs_resume, jobs_positions, etc.)
 * @param {string} key — facts key
 * @param {Object} fields — { fieldName: { label, value, type?: 'text'|'textarea' } }
 * @param {Function} onSaved — callback after save, receives updated data object
 */
export function showEditModal(category, key, fields, onSaved) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';

  const rows = Object.entries(fields).map(([name, f]) => {
    const escaped = escapeHtml(f.value || '');
    if (f.type === 'textarea') {
      return `<div class="form-group"><label class="form-label">${f.label}</label><textarea class="form-input jm-edit-field" data-field="${name}" rows="3">${escaped}</textarea></div>`;
    }
    return `<div class="form-group"><label class="form-label">${f.label}</label><input class="form-input jm-edit-field" data-field="${name}" value="${escaped}"></div>`;
  }).join('');

  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Редактирование</div>
    ${rows}
    <div class="modal-actions">
      <button class="btn-secondary jm-edit-cancel">Отмена</button>
      <button class="btn-primary jm-edit-save">Сохранить</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('.jm-edit-cancel').addEventListener('click', () => overlay.remove());

  overlay.querySelector('.jm-edit-save').addEventListener('click', async () => {
    const updated = {};
    overlay.querySelectorAll('.jm-edit-field').forEach(el => {
      const name = el.dataset.field;
      const val = el.value;
      // Convert textarea "bullets" field to array
      if (name === 'bullets') {
        updated[name] = val.split('\n').map(l => l.trim()).filter(Boolean);
      } else {
        updated[name] = val;
      }
    });
    await invoke('memory_remember', { category, key, value: JSON.stringify(updated) }).catch(() => {});
    overlay.remove();
    if (onSaved) onSaved(updated);
  });
}

/** SVG pencil icon for edit buttons */
export const EDIT_ICON = '<svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M11.5 1.5l3 3L5 14H2v-3L11.5 1.5z"/></svg>';
