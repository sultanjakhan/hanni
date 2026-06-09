// ── js/routine-node-sport.js — pick exercises OR ready-made sets for a sport step ──
// Opened from a 'sport' node in the routine graph. Two modes:
//  • Сеты      — pick one or more ready workout templates (e.g. Гибкость уровни) → each logs a workout
//  • Упражнения — search the catalog, multi-select single exercises → one workout
// Uses existing commands only (create_workout_from_template / create_workout_template),
// throwaway template for the ad-hoc case leaves no orphan behind.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { loadCategories } from './calendar-categories.js';

let _cat = null, _tpl = null;
async function catalog() {
  if (!_cat) _cat = await invoke('get_exercise_catalog', { search: null }).catch(() => []);
  return _cat;
}
async function templates() {
  if (!_tpl) _tpl = await invoke('get_workout_templates', { search: null }).catch(() => []);
  return _tpl;
}

const CATS = [
  { id: 'stretching', label: 'Растяжка' }, { id: 'strength', label: 'Силовые' },
  { id: 'cardio', label: 'Кардио' }, { id: 'plyometrics', label: 'Плиометрика' }, { id: '', label: 'Все' },
];
const DIFFS = [
  { id: '', label: 'Любая' }, { id: 'easy', label: 'Лёгкий' },
  { id: 'medium', label: 'Средний' }, { id: 'hard', label: 'Сложный' },
];
function guessCat(title) {
  const t = (title || '').toLowerCase();
  if (/силов|качат|жим|тяг|присед|strength|gym/.test(t)) return 'strength';
  if (/кардио|cardio|\bбег/.test(t)) return 'cardio';
  return 'stretching';
}

export function openSportPicker(node, onDone) {
  let mode = 'sets';                 // 'sets' | 'catalog'
  let activeCat = guessCat(node.title);
  let activeDiff = '';
  const pickedTpl = new Map();       // template_id -> { id, name }
  const pickedEx = new Map();        // catalog_id  -> { id, name }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal sp-modal">
    <div class="modal-title">🔥 ${escapeHtml(node.title)}</div>
    <div class="dev-filters" id="sp-mode">
      <button class="dev-filter-btn active" data-mode="sets">Готовые сеты</button>
      <button class="dev-filter-btn" data-mode="catalog">Упражнения</button>
    </div>
    <div id="sp-body"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="sp-cancel">Отмена</button>
      <button class="btn-primary" id="sp-save">Записать</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#sp-cancel').onclick = () => overlay.remove();
  const bodyEl = overlay.querySelector('#sp-body');

  const chipRow = (picked, onRm) => `<div class="sp-picked">${[...picked.values()].map(p =>
    `<span class="sp-chip">${escapeHtml(p.name)}<button data-rm="${p.id}" title="Убрать">×</button></span>`).join('')}</div>`;

  // ── Sets mode: ready templates, multi-select ──
  async function renderSets() {
    const all = await templates();
    bodyEl.innerHTML = chipRow(pickedTpl) + `<div class="sp-list" id="sp-list"></div>`;
    const listEl = bodyEl.querySelector('#sp-list');
    listEl.innerHTML = all.length ? all.map(t =>
      `<div class="sp-item${pickedTpl.has(t.id) ? ' on' : ''}" data-id="${t.id}" data-name="${escapeHtml(t.name)}">
        <span class="sp-item-name">${escapeHtml(t.name)}</span>
        <span class="sp-item-meta">${t.exercise_count || 0} упр · ${escapeHtml(t.difficulty || '')}</span>
      </div>`).join('') : '<div class="rt-add-empty">Нет готовых сетов — создай шаблон в Спорте</div>';
    wireList(listEl, pickedTpl, renderSets);
  }

  // ── Catalog mode: single exercises with category + difficulty filters ──
  async function renderCatalog() {
    const all = await catalog();
    bodyEl.innerHTML = `
      <div class="dev-filters" id="sp-cats">${CATS.map(c => `<button class="dev-filter-btn${c.id === activeCat ? ' active' : ''}" data-cat="${c.id}">${c.label}</button>`).join('')}</div>
      <div class="dev-filters" id="sp-diffs">${DIFFS.map(d => `<button class="dev-filter-btn${d.id === activeDiff ? ' active' : ''}" data-diff="${d.id}">${d.label}</button>`).join('')}</div>
      <input class="form-input" id="sp-search" placeholder="Поиск упражнения…" autocomplete="off">
      ${chipRow(pickedEx)}
      <div class="sp-list" id="sp-list"></div>`;
    const listEl = bodyEl.querySelector('#sp-list');
    const searchEl = bodyEl.querySelector('#sp-search');
    const draw = () => {
      const lc = searchEl.value.trim().toLowerCase();
      const rows = all.filter(e => !activeCat || e.category === activeCat)
        .filter(e => !activeDiff || e.difficulty === activeDiff)
        .filter(e => !lc || e.name.toLowerCase().includes(lc)).slice(0, 40);
      listEl.innerHTML = rows.map(e =>
        `<div class="sp-item${pickedEx.has(e.id) ? ' on' : ''}" data-id="${e.id}" data-name="${escapeHtml(e.name)}">
          <span class="sp-item-name">${escapeHtml(e.name)}</span>
          <span class="sp-item-meta">${escapeHtml(e.muscle_group || '')} · ${escapeHtml(e.difficulty || '')}</span>
        </div>`).join('') || '<div class="rt-add-empty">Ничего не найдено</div>';
      wireList(listEl, pickedEx, draw);
    };
    searchEl.oninput = draw;
    bodyEl.querySelectorAll('#sp-cats [data-cat]').forEach(b => b.onclick = () => { activeCat = b.dataset.cat; renderCatalog(); });
    bodyEl.querySelectorAll('#sp-diffs [data-diff]').forEach(b => b.onclick = () => { activeDiff = b.dataset.diff; renderCatalog(); });
    draw();
  }

  function wireList(listEl, picked, redraw) {
    listEl.querySelectorAll('[data-id]').forEach(it => it.onclick = () => {
      const id = parseInt(it.dataset.id);
      if (picked.has(id)) picked.delete(id); else picked.set(id, { id, name: it.dataset.name });
      redraw();
    });
    bodyEl.querySelectorAll('.sp-picked [data-rm]').forEach(b => b.onclick = () => { picked.delete(parseInt(b.dataset.rm)); redraw(); });
  }

  const renderBody = () => (mode === 'sets' ? renderSets() : renderCatalog());
  overlay.querySelectorAll('#sp-mode [data-mode]').forEach(b => b.onclick = () => {
    mode = b.dataset.mode;
    overlay.querySelectorAll('#sp-mode [data-mode]').forEach(x => x.classList.toggle('active', x.dataset.mode === mode));
    renderBody();
  });
  renderBody();

  overlay.querySelector('#sp-save').onclick = async () => {
    try {
      let logged = 0;
      if (mode === 'sets') {
        for (const t of pickedTpl.values()) { await invoke('create_workout_from_template', { templateId: t.id }); logged++; }
      } else if (pickedEx.size) {
        const items = [...pickedEx.values()].map(p => ({
          exercise_catalog_id: p.id, name: p.name, sets: 1, reps: 0, weight_kg: 0, duration_seconds: 30, rest_seconds: 15,
        }));
        const today = new Date().toISOString().slice(0, 10);
        const tid = await invoke('create_workout_template', {
          name: `${node.title} ${today}`, templateType: 'yoga', difficulty: 'easy',
          targetMuscleGroups: '', notes: 'Из рутины', exerciseItems: items,
        });
        await invoke('create_workout_from_template', { templateId: tid });
        await invoke('delete_workout_template', { id: tid }).catch(() => {});
        logged = 1;
      }
      if (logged) {
        // Mirror the logged workout into the calendar so it shows there too.
        try {
          const cats = await loadCategories();
          const color = cats.find(c => c.name === 'Спорт')?.color || '#d9730d';
          await invoke('create_event', {
            title: `🏋️ ${node.title}`, description: 'Тренировка из рутины',
            date: new Date().toISOString().slice(0, 10), time: new Date().toTimeString().slice(0, 5),
            durationMinutes: 15, category: 'Спорт', color, priority: null, linkedTab: 'sports',
          });
          window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
        } catch (_) { /* calendar mirror is best-effort */ }
      }
      overlay.remove();
      if (logged && onDone) onDone(logged);
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
