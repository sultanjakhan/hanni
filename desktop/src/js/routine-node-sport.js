// ── js/routine-node-sport.js — pick catalog exercises for a sport routine step ──
// Opened from a 'sport' node in the routine graph. Lets the user search the
// exercise catalog, multi-select, and log a workout for today. Implemented with
// existing commands: temp template → create_workout_from_template → delete temp
// (exercises stay on the workout, no orphan template left behind).
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

let _cat = null;
async function catalog() {
  if (!_cat) _cat = await invoke('get_exercise_catalog', { search: null }).catch(() => []);
  return _cat;
}

// Category chips — picker opens pre-filtered to what the step is about.
const CATS = [
  { id: 'stretching', label: 'Растяжка' },
  { id: 'strength', label: 'Силовые' },
  { id: 'cardio', label: 'Кардио' },
  { id: 'plyometrics', label: 'Плиометрика' },
  { id: '', label: 'Все' },
];
function guessCat(title) {
  const t = (title || '').toLowerCase();
  if (/силов|качат|жим|тяг|присед|strength|gym/.test(t)) return 'strength';
  if (/кардио|cardio|\bбег/.test(t)) return 'cardio';
  return 'stretching'; // растяжка / разминка / зарядка → растяжки
}

const DIFFS = [
  { id: '', label: 'Любая' },
  { id: 'easy', label: 'Лёгкий' },
  { id: 'medium', label: 'Средний' },
  { id: 'hard', label: 'Сложный' },
];

export function openSportPicker(node, onDone) {
  const picked = new Map(); // catalog_id -> { id, name }
  let activeCat = guessCat(node.title);
  let activeDiff = '';
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal sp-modal">
    <div class="modal-title">🔥 ${escapeHtml(node.title)} — упражнения из каталога</div>
    <div class="dev-filters" id="sp-cats">
      ${CATS.map(c => `<button class="dev-filter-btn${c.id === activeCat ? ' active' : ''}" data-cat="${c.id}">${c.label}</button>`).join('')}
    </div>
    <div class="dev-filters" id="sp-diffs">
      ${DIFFS.map(d => `<button class="dev-filter-btn${d.id === activeDiff ? ' active' : ''}" data-diff="${d.id}">${d.label}</button>`).join('')}
    </div>
    <input class="form-input" id="sp-search" placeholder="Поиск упражнения…" autocomplete="off">
    <div class="sp-picked" id="sp-picked"></div>
    <div class="sp-list" id="sp-list"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="sp-cancel">Отмена</button>
      <button class="btn-primary" id="sp-save">Записать тренировку</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#sp-cancel').onclick = () => overlay.remove();

  const listEl = overlay.querySelector('#sp-list');
  const pickedEl = overlay.querySelector('#sp-picked');
  const searchEl = overlay.querySelector('#sp-search');

  const drawPicked = () => {
    pickedEl.innerHTML = [...picked.values()].map(p =>
      `<span class="sp-chip">${escapeHtml(p.name)}<button data-rm="${p.id}" title="Убрать">×</button></span>`).join('');
    pickedEl.querySelectorAll('[data-rm]').forEach(b =>
      b.onclick = () => { picked.delete(parseInt(b.dataset.rm)); drawPicked(); drawList(); });
  };

  const drawList = async () => {
    const all = await catalog();
    const lc = searchEl.value.trim().toLowerCase();
    const rows = all
      .filter(e => !activeCat || e.category === activeCat)
      .filter(e => !activeDiff || e.difficulty === activeDiff)
      .filter(e => !lc || e.name.toLowerCase().includes(lc))
      .slice(0, 40);
    listEl.innerHTML = rows.map(e =>
      `<div class="sp-item${picked.has(e.id) ? ' on' : ''}" data-id="${e.id}" data-name="${escapeHtml(e.name)}">
        <span class="sp-item-name">${escapeHtml(e.name)}</span>
        <span class="sp-item-meta">${escapeHtml(e.muscle_group || '')} · ${escapeHtml(e.difficulty || '')}</span>
      </div>`).join('') || '<div class="rt-add-empty">Ничего не найдено</div>';
    listEl.querySelectorAll('[data-id]').forEach(it => it.onclick = () => {
      const id = parseInt(it.dataset.id);
      if (picked.has(id)) picked.delete(id); else picked.set(id, { id, name: it.dataset.name });
      drawPicked(); drawList();
    });
  };
  searchEl.oninput = drawList;
  overlay.querySelectorAll('#sp-cats [data-cat]').forEach(btn => btn.onclick = () => {
    activeCat = btn.dataset.cat;
    overlay.querySelectorAll('#sp-cats [data-cat]').forEach(b => b.classList.toggle('active', b.dataset.cat === activeCat));
    drawList();
  });
  overlay.querySelectorAll('#sp-diffs [data-diff]').forEach(btn => btn.onclick = () => {
    activeDiff = btn.dataset.diff;
    overlay.querySelectorAll('#sp-diffs [data-diff]').forEach(b => b.classList.toggle('active', b.dataset.diff === activeDiff));
    drawList();
  });
  drawPicked();
  drawList();

  overlay.querySelector('#sp-save').onclick = async () => {
    if (picked.size === 0) { overlay.remove(); return; }
    const items = [...picked.values()].map(p => ({
      exercise_catalog_id: p.id, name: p.name,
      sets: 1, reps: 0, weight_kg: 0, duration_seconds: 30, rest_seconds: 15,
    }));
    const today = new Date().toISOString().slice(0, 10);
    try {
      const tid = await invoke('create_workout_template', {
        name: `${node.title} ${today}`, templateType: 'yoga', difficulty: 'easy',
        targetMuscleGroups: '', notes: 'Из рутины', exerciseItems: items,
      });
      await invoke('create_workout_from_template', { templateId: tid });
      await invoke('delete_workout_template', { id: tid }).catch(() => {});
      overlay.remove();
      if (onDone) onDone(picked.size);
    } catch (err) { alert('Ошибка: ' + err); }
  };
}
