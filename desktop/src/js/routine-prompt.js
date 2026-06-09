// ── js/routine-prompt.js — Promise-based input modal for the routine editor ──
// Replaces window.prompt (native dialog breaks the app's modal language).
// Resolves the entered string, or null on cancel / overlay click / Escape.
import { escapeHtml } from './utils.js';

export function promptModal({ title, value = '', placeholder = '', type = 'text' }) {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal rt-prompt-modal">
      <div class="modal-title">${escapeHtml(title)}</div>
      <input class="rt-prompt-input" type="${type}" value="${escapeHtml(String(value))}"
             placeholder="${escapeHtml(placeholder)}">
      <div class="modal-actions">
        <button class="btn-secondary" data-act="cancel">Отмена</button>
        <button class="btn-primary" data-act="ok">OK</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    const input = overlay.querySelector('.rt-prompt-input');
    const close = (val) => { overlay.remove(); resolve(val); };
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(null); });
    overlay.querySelector('[data-act="cancel"]').addEventListener('click', () => close(null));
    overlay.querySelector('[data-act="ok"]').addEventListener('click', () => close(input.value));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') close(input.value);
      if (e.key === 'Escape') close(null);
    });
    input.focus();
    input.select();
  });
}
