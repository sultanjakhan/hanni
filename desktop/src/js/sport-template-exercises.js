// ── sport-template-exercises.js — Dynamic exercise rows with autocomplete ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

let _acCache = null;

async function getAutocomplete(query) {
  if (!_acCache) _acCache = await invoke('get_exercise_catalog', { search: null }).catch(() => []);
  if (!query) return _acCache.slice(0, 8);
  const lc = query.toLowerCase();
  return _acCache.filter(e => e.name.toLowerCase().includes(lc)).slice(0, 8);
}

export function invalidateAcCache() { _acCache = null; }

export function createExerciseRows(container, initial) {
  const rows = initial || [{ name: '', sets: 3, reps: 10, weight_kg: 0, rest_seconds: 60 }];
  render(container, rows);
}

function render(container, rows) {
  container.innerHTML = rows.map((r, i) => `
    <div class="exercise-row" data-idx="${i}">
      <div class="exercise-row-main">
        <div class="exercise-ac-wrap">
          <input class="form-input exercise-name" placeholder="Упражнение" value="${escapeHtml(r.name || '')}" data-cat-id="${r.exercise_catalog_id || ''}">
          <div class="exercise-ac-list" style="display:none"></div>
        </div>
        <input class="form-input exercise-sets" type="number" min="1" value="${r.sets || 3}" title="Подходы" placeholder="П" style="width:52px">
        <span class="form-hint">×</span>
        <input class="form-input exercise-reps" type="number" min="1" value="${r.reps || 10}" title="Повторы" placeholder="Р" style="width:52px">
        <input class="form-input exercise-weight" type="number" min="0" step="0.5" value="${r.weight_kg || 0}" title="Вес (кг)" placeholder="Вес" style="width:60px">
        <span class="form-hint">кг</span>
        <input class="form-input exercise-rest" type="number" min="0" value="${r.rest_seconds || 60}" title="Отдых (с)" placeholder="Отдых" style="width:60px">
        <span class="form-hint">с</span>
        <button class="btn-icon exercise-remove" title="Удалить">✕</button>
      </div>
    </div>`).join('') + `<button class="btn-secondary exercise-add-row" style="margin-top:6px">+ Упражнение</button>`;

  container.querySelector('.exercise-add-row')?.addEventListener('click', () => {
    rows.push({ name: '', sets: 3, reps: 10, weight_kg: 0, rest_seconds: 60 });
    render(container, rows);
  });
  container.querySelectorAll('.exercise-remove').forEach(btn => {
    btn.onclick = () => { rows.splice(parseInt(btn.closest('.exercise-row').dataset.idx), 1); render(container, rows); };
  });
  container.querySelectorAll('.exercise-name').forEach(input => wireAutocomplete(input, rows, container));
  syncInputs(container, rows);
}

function syncInputs(container, rows) {
  container.querySelectorAll('.exercise-row').forEach((rowEl, i) => {
    const r = rows[i]; if (!r) return;
    rowEl.querySelector('.exercise-sets').onchange = (e) => { r.sets = parseInt(e.target.value) || 3; };
    rowEl.querySelector('.exercise-reps').onchange = (e) => { r.reps = parseInt(e.target.value) || 10; };
    rowEl.querySelector('.exercise-weight').onchange = (e) => { r.weight_kg = parseFloat(e.target.value) || 0; };
    rowEl.querySelector('.exercise-rest').onchange = (e) => { r.rest_seconds = parseInt(e.target.value) || 60; };
  });
}

function wireAutocomplete(input, rows, container) {
  const idx = parseInt(input.closest('.exercise-row').dataset.idx);
  const list = input.nextElementSibling;
  input.oninput = async () => {
    const q = input.value.trim();
    const matches = await getAutocomplete(q);
    if (!matches.length) { list.style.display = 'none'; return; }
    list.innerHTML = matches.map(m => `<div class="exercise-ac-item" data-id="${m.id}">${escapeHtml(m.name)}</div>`).join('');
    list.style.display = '';
    list.querySelectorAll('.exercise-ac-item').forEach(item => item.onclick = () => {
      rows[idx].name = item.textContent;
      rows[idx].exercise_catalog_id = parseInt(item.dataset.id);
      input.value = item.textContent;
      input.dataset.catId = item.dataset.id;
      list.style.display = 'none';
    });
  };
  input.onblur = () => setTimeout(() => { list.style.display = 'none'; }, 200);
  input.onchange = () => { rows[idx].name = input.value.trim(); };
}

export function collectExerciseItems(container) {
  const items = [];
  container.querySelectorAll('.exercise-row').forEach(rowEl => {
    const name = rowEl.querySelector('.exercise-name')?.value?.trim();
    if (!name) return;
    items.push({
      name,
      exercise_catalog_id: parseInt(rowEl.querySelector('.exercise-name')?.dataset?.catId) || null,
      sets: parseInt(rowEl.querySelector('.exercise-sets')?.value) || 3,
      reps: parseInt(rowEl.querySelector('.exercise-reps')?.value) || 10,
      weight_kg: parseFloat(rowEl.querySelector('.exercise-weight')?.value) || 0,
      rest_seconds: parseInt(rowEl.querySelector('.exercise-rest')?.value) || 60,
    });
  });
  return items;
}
