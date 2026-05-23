// calendar-event-templates.js — Quick templates accordion for the
// "New event" modal. Click a template → prefill title/duration/category;
// some templates also surface an inline picker (медитация, лекарство)
// or hand-off to a specialised modal (еда, закупка).

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { showCookingLogModal } from './food-cooking-log.js';
import { showShoppingPicker } from './shopping-list-modal.js';
import { itemsToDescription } from './shopping-list.js';

// Source of truth for what shows up in the accordion. Editing this list
// here (vs DB) is intentional — these are app-level UX shortcuts, not
// user data; CRUD on them doesn't belong in the catalogue tables.
const TEMPLATES = [
  { id: 'cook',    icon: '🍳', label: 'Готовка',     title: 'Готовка',          dur: 30,  cat: 'Готовка',    handoff: 'cook' },
  { id: 'shower',  icon: '🚿', label: 'Душ',         title: 'Душ',              dur: 15,  cat: 'Быт' },
  { id: 'toilet',  icon: '🚽', label: 'Туалет',      title: 'Туалет',           dur: 5,   cat: 'Быт' },
  { id: 'medit',   icon: '🧘', label: 'Медитация',   title: 'Медитация',        dur: 10,  cat: 'Спорт',      spec: 'medit' },
  { id: 'book',    icon: '📚', label: 'Чтение',      title: 'Чтение',           dur: 30,  cat: 'Учёба' },
  { id: 'meds',    icon: '💊', label: 'Лекарство',   title: 'Принять',          dur: 5,   cat: 'Быт',        spec: 'meds' },
  { id: 'shop',    icon: '🛒', label: 'Закупка',     title: 'Закупка продуктов', dur: 60, cat: 'Быт',       handoff: 'shop' },
  { id: 'laundry', icon: '🧺', label: 'Постирать',   title: 'Постирать',        dur: 10,  cat: 'Быт' },
  { id: 'trash',   icon: '🗑', label: 'Мусор',       title: 'Вынести мусор',    dur: 5,   cat: 'Быт' },
];

const MEDIT_TECHNIQUES = [
  { id: 'box',     label: 'Box 4-4-4-4',    dur: 5,  hint: 'фокус' },
  { id: 'relax',   label: '4-7-8',          dur: 10, hint: 'расслабление' },
  { id: 'coh',     label: 'Coherent 5-5',   dur: 10, hint: 'баланс' },
];

export function renderTemplatesAccordion() {
  const chips = TEMPLATES.map(t => `
    <button type="button" class="evm-tpl-chip" data-tpl="${t.id}">
      <span class="evm-tpl-ico">${t.icon}</span><span>${escapeHtml(t.label)}</span>
    </button>`).join('');
  return `<details class="evm-templates" open>
    <summary class="evm-templates-sum">⚡ Быстрые шаблоны</summary>
    <div class="evm-tpl-row">${chips}</div>
    <div class="evm-tpl-spec" id="evm-tpl-spec" hidden></div>
  </details>`;
}

// Wire the accordion buttons after the modal HTML is in the DOM.
// Some templates close the event modal (cook, shop) — others mutate
// the existing fields in place.
export function bindTemplates(overlay, ctx) {
  const specEl = overlay.querySelector('#evm-tpl-spec');
  overlay.querySelectorAll('.evm-tpl-chip').forEach(btn => {
    btn.addEventListener('click', () => {
      const tpl = TEMPLATES.find(t => t.id === btn.dataset.tpl);
      if (!tpl) return;
      overlay.querySelectorAll('.evm-tpl-chip').forEach(c => c.classList.remove('active'));
      btn.classList.add('active');
      handleTemplate(tpl, overlay, specEl, ctx);
    });
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

  if (tpl.spec === 'medit') {
    specEl.hidden = false;
    specEl.innerHTML = `<div class="evm-tpl-spec-label">Техника дыхания</div>
      <div class="evm-tpl-spec-row">${MEDIT_TECHNIQUES.map(m => `
        <button type="button" class="evm-tpl-chip" data-mt="${m.id}">
          <span>${escapeHtml(m.label)}</span>
          <span class="evm-tpl-chip-hint">${escapeHtml(m.hint)} · ${m.dur}м</span>
        </button>`).join('')}</div>`;
    specEl.querySelectorAll('[data-mt]').forEach(b => {
      b.addEventListener('click', () => {
        const m = MEDIT_TECHNIQUES.find(x => x.id === b.dataset.mt);
        if (!m) return;
        specEl.querySelectorAll('[data-mt]').forEach(x => x.classList.remove('active'));
        b.classList.add('active');
        const titleEl = overlay.querySelector('#evm-title');
        if (titleEl) titleEl.value = `Медитация · ${m.label}`;
        const durEl = overlay.querySelector('#evm-dur');
        if (durEl) durEl.value = m.dur;
      });
    });
  }

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
