// calendar-event-templates.js — Quick templates accordion for the
// "New event" modal. Click a template → prefill title/duration/category;
// some templates also surface an inline picker (медитация, лекарство)
// or hand-off to a specialised modal (еда, закупка).

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { showCookingLogModal, showCookWhatModal } from './food-cooking-log.js';
import { showShoppingPicker } from './shopping-list-modal.js';
import { itemsToDescription } from './shopping-list.js';

// Source of truth for what shows up in the accordion. Editing this list
// here (vs DB) is intentional — these are app-level UX shortcuts, not
// user data; CRUD on them doesn't belong in the catalogue tables.
const TEMPLATES = [
  { id: 'cook',    icon: '🍳', label: 'Готовка',     title: 'Готовка',          dur: 30, cat: 'Готовка', tab: 'food',    handoff: 'cook' },
  { id: 'cookwhat',icon: '🍲', label: 'Что приготовить', title: 'Готовка',      dur: 30, cat: 'Готовка', tab: 'food',    handoff: 'cookwhat' },
  { id: 'workout', icon: '🏋️', label: 'Тренировка',  title: 'Тренировка',       dur: 60, cat: 'Спорт',   tab: 'sports' },
  { id: 'shower',  icon: '🚿', label: 'Душ',         title: 'Душ',              dur: 15, cat: 'Быт' },
  { id: 'toilet',  icon: '🚽', label: 'Туалет',      title: 'Туалет',           dur: 5,  cat: 'Быт' },
  { id: 'book',    icon: '📚', label: 'Чтение',      title: 'Чтение',           dur: 30, cat: 'Учёба' },
  { id: 'meds',    icon: '💊', label: 'Лекарство',   title: 'Принять',          dur: 5,  cat: 'Быт',    tab: 'health', spec: 'meds' },
  { id: 'shop',    icon: '🛒', label: 'Закупка',     title: 'Закупка продуктов', dur: 60, cat: 'Быт',   tab: 'home',   handoff: 'shop' },
  { id: 'laundry', icon: '🧺', label: 'Постирать',   title: 'Постирать',        dur: 10, cat: 'Быт' },
  { id: 'trash',   icon: '🗑', label: 'Мусор',       title: 'Вынести мусор',    dur: 5,  cat: 'Быт' },
];

export function renderTemplatesAccordion() {
  const opts = TEMPLATES.map(t =>
    `<option value="${t.id}">${t.icon} ${escapeHtml(t.label)}</option>`
  ).join('');
  return `<div class="form-row evm-tpl-row">
    <select class="form-select" id="evm-tpl-sel">
      <option value="">⚡ Шаблон — выбери для авто-заполнения…</option>
      ${opts}
    </select>
    <div class="evm-tpl-spec" id="evm-tpl-spec" hidden></div>
  </div>`;
}

// Wire the template select after the modal HTML is in the DOM.
// Some templates close the event modal (cook, shop) — others mutate
// the existing fields in place.
export function bindTemplates(overlay, ctx) {
  const sel = overlay.querySelector('#evm-tpl-sel');
  const specEl = overlay.querySelector('#evm-tpl-spec');
  if (!sel) return;
  sel.addEventListener('change', () => {
    const tpl = TEMPLATES.find(t => t.id === sel.value);
    if (!tpl) {
      if (specEl) { specEl.hidden = true; specEl.innerHTML = ''; }
      const prefix = overlay.querySelector('#evm-tpl-prefix');
      if (prefix) { prefix.textContent = ''; prefix.hidden = true; }
      return;
    }
    handleTemplate(tpl, overlay, specEl, ctx);
  });
}

function applyPrefill(overlay, tpl) {
  const titleEl = overlay.querySelector('#evm-title');
  const durEl = overlay.querySelector('#evm-dur');
  const catEl = overlay.querySelector('#evm-cat');
  if (titleEl && !titleEl.value.trim()) titleEl.value = tpl.title;
  if (durEl) durEl.value = tpl.dur;
  if (catEl && Array.from(catEl.options).some(o => o.value === tpl.cat)) {
    catEl.value = tpl.cat;
  }
  if (tpl.tab) {
    const tabSel = overlay.querySelector('#evm-linked-tab');
    if (tabSel && Array.from(tabSel.options).some(o => o.value === tpl.tab)) {
      tabSel.value = tpl.tab;
    }
  }
  const prefix = overlay.querySelector('#evm-tpl-prefix');
  if (prefix) {
    prefix.textContent = tpl.icon || '';
    prefix.hidden = !tpl.icon;
  }
}

function handleTemplate(tpl, overlay, specEl, ctx) {
  specEl.hidden = true; specEl.innerHTML = '';

  if (tpl.handoff === 'cook') {
    const date = overlay.querySelector('#evm-date')?.value;
    overlay.remove();
    showCookingLogModal(date, () =>
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')));
    return;
  }
  if (tpl.handoff === 'cookwhat') {
    const date = overlay.querySelector('#evm-date')?.value;
    overlay.remove();
    showCookWhatModal(date, () =>
      window.dispatchEvent(new CustomEvent('hanni:calendar-refresh')));
    return;
  }
  if (tpl.handoff === 'shop') {
    applyPrefill(overlay, tpl);
    showShoppingPicker(async (picked) => {
      if (!picked || !picked.length) return;
      const descEl = overlay.querySelector('#evm-desc');
      if (descEl) descEl.value = itemsToDescription(picked);
      // Remember the picked ids so the modal's Save handler can mark
      // them bought_at after a successful create_event.
      ctx.shoppingPickedIds = picked.map(p => p.id);
    });
    return;
  }

  applyPrefill(overlay, tpl);

  if (tpl.spec === 'meds') {
    specEl.hidden = false;
    specEl.innerHTML = `<div class="evm-tpl-spec-label">Что приняли?</div>
      <input class="form-input" id="evm-tpl-meds" placeholder="Витамин D / Магний / …" list="evm-tpl-meds-dl">
      <datalist id="evm-tpl-meds-dl"></datalist>`;
    // Populate datalist from past "Принять *" events — async so the
    // accordion shows immediately and suggestions stream in.
    fillMedsSuggestions(specEl);
    const inputEl = specEl.querySelector('#evm-tpl-meds');
    inputEl.focus();
    inputEl.addEventListener('input', () => {
      const v = inputEl.value.trim();
      const titleEl = overlay.querySelector('#evm-title');
      if (titleEl) titleEl.value = v ? `Принять: ${v}` : 'Принять';
    });
  }
}

async function fillMedsSuggestions(specEl) {
  try {
    const events = await invoke('get_all_events').catch(() => []);
    const seen = new Set();
    const opts = [];
    for (const e of events || []) {
      const t = (e.title || '').trim();
      if (!t.toLowerCase().startsWith('принять')) continue;
      const after = t.replace(/^принять[:\s]*/i, '').trim();
      if (!after || seen.has(after.toLowerCase())) continue;
      seen.add(after.toLowerCase()); opts.push(after);
      if (opts.length >= 12) break;
    }
    const dl = specEl.querySelector('#evm-tpl-meds-dl');
    if (dl) dl.innerHTML = opts.map(o => `<option value="${escapeHtml(o)}">`).join('');
  } catch { /* suggestions are best-effort */ }
}
