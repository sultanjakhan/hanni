// ── js/prompt-modal.js — Promise-based input modal (app-styled prompt()) ──
// Companion to confirmModal in utils.js. Native window.prompt breaks the
// app's modal language; this renders the standard overlay + .modal pattern.
//   Single field: promptModal({ title, value, placeholder, type }) → string | null
//   Multi-field:  promptModal({ title, fields: [{ key, label, value, placeholder, type }] })
//                 → { key: value, … } | null
// Cancel / overlay click / Escape resolve null.
import { escapeHtml } from './utils.js';

export function promptModal({ title, value = '', placeholder = '', type = 'text', fields = null }) {
  const defs = fields || [{ key: '_single', value, placeholder, type }];
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal pm-modal">
      <div class="modal-title">${escapeHtml(title)}</div>
      ${defs.map((f, i) => `
        ${f.label ? `<label class="pm-label" for="pm-f${i}">${escapeHtml(f.label)}</label>` : ''}
        <input class="pm-input" id="pm-f${i}" data-key="${escapeHtml(f.key)}"
               type="${f.type || 'text'}" value="${escapeHtml(String(f.value ?? ''))}"
               placeholder="${escapeHtml(f.placeholder || '')}">`).join('')}
      <div class="modal-actions">
        <button class="btn-secondary" data-act="cancel">Отмена</button>
        <button class="btn-primary" data-act="ok">OK</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    const inputs = [...overlay.querySelectorAll('.pm-input')];
    const close = (val) => { overlay.remove(); resolve(val); };
    const submit = () => {
      if (!fields) return close(inputs[0].value);
      close(Object.fromEntries(inputs.map(inp => [inp.dataset.key, inp.value])));
    };
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(null); });
    overlay.querySelector('[data-act="cancel"]').addEventListener('click', () => close(null));
    overlay.querySelector('[data-act="ok"]').addEventListener('click', submit);
    inputs.forEach(inp => inp.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') submit();
      if (e.key === 'Escape') close(null);
    }));
    inputs[0].focus();
    inputs[0].select();
  });
}
