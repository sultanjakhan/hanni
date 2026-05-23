// ── food-blacklist-add.js — Popover to add a preference entry of one type ──
import { escapeHtml } from './utils.js';

let _pop = null;
export function closeAddPopover() { if (_pop) { _pop.remove(); _pop = null; } }

// options: [{ value, label, catalogId? }] already scoped to the target type.
// onPick(option) is called on selection; the caller does the actual add.
export function openAddPopover(anchorEl, { placeholder = 'Поиск…', options }, onPick) {
  closeAddPopover();
  const pop = document.createElement('div');
  pop.className = 'bl-add-pop';
  pop.innerHTML = `<input class="form-input bl-add-input" placeholder="${escapeHtml(placeholder)}" autocomplete="off">
    <div class="bl-ac"></div>`;
  document.body.appendChild(pop);
  _pop = pop;

  const rect = anchorEl.getBoundingClientRect();
  pop.style.left = `${Math.min(rect.left, window.innerWidth - 260)}px`;
  pop.style.top = `${rect.bottom + 4}px`;

  const input = pop.querySelector('.bl-add-input');
  const ac = pop.querySelector('.bl-ac');

  function render() {
    const q = input.value.trim().toLowerCase();
    const hits = (q ? options.filter(o => o.label.toLowerCase().includes(q)) : options).slice(0, 30);
    ac.innerHTML = hits.length
      ? hits.map((h, i) => `<div class="bl-ac-row" data-i="${i}">${escapeHtml(h.label)}${h.kindLabel ? `<small>${escapeHtml(h.kindLabel)}</small>` : ''}</div>`).join('')
      : '<div class="bl-empty" style="padding:6px 10px">ничего не найдено</div>';
    ac.querySelectorAll('.bl-ac-row').forEach(row =>
      row.onmousedown = (ev) => { ev.preventDefault(); const o = hits[parseInt(row.dataset.i)]; closeAddPopover(); onPick(o); });
  }

  input.oninput = render;
  render();
  input.focus();

  setTimeout(() => document.addEventListener('mousedown', function onOutside(ev) {
    if (_pop && !_pop.contains(ev.target)) { closeAddPopover(); document.removeEventListener('mousedown', onOutside); }
  }), 10);
}
