// ── sport-picker.js — Guided exercise picker (muscle → difficulty → equipment) ──
import { chips } from './utils.js';
import { renderExerciseCard } from './sport-catalog-card.js';
import {
  MUSCLE_GROUPS, DIFFICULTIES, EQUIP_MODES,
  loadExerciseCatalog, getCatalogCache, loadExerciseFacets,
  matchMuscle, matchDifficulty, matchEquipment,
} from './sport-catalog-filters.js';

const RESULT_CAP = 100;

export async function renderPickerPane(el) {
  const F = { muscle: 'all', difficulty: 'all', equipMode: 'any', equipment: new Set() };

  await loadExerciseCatalog();
  const facets = await loadExerciseFacets();

  function getFiltered() {
    return getCatalogCache().filter(ex =>
      matchMuscle(ex, F.muscle) && matchDifficulty(ex, F.difficulty)
      && matchEquipment(ex, F.equipMode, F.equipment));
  }

  function equipListHtml() {
    if (F.equipMode !== 'with' || !facets.equipment.length) return '';
    const items = facets.equipment.map(e =>
      `<button class="rf-chip${F.equipment.has(e) ? ' active' : ''}" data-equip="${e}">${e}</button>`
    ).join('');
    return `<div class="rf-section"><span class="rf-title">Инвентарь</span><div class="rf-chip-row">${items}</div></div>`;
  }

  function stepsHtml() {
    return `
      <div class="rf-section"><span class="rf-title">1 · Группа мышц</span>
        <div class="rf-chip-row">${chips(MUSCLE_GROUPS, F.muscle, 'muscle')}</div></div>
      <div class="rf-section"><span class="rf-title">2 · Сложность</span>
        <div class="rf-chip-row">${chips(DIFFICULTIES, F.difficulty, 'difficulty')}</div></div>
      <div class="rf-section"><span class="rf-title">3 · Оборудование</span>
        <div class="rf-chip-row">${chips(EQUIP_MODES, F.equipMode, 'equipMode')}</div></div>
      ${equipListHtml()}`;
  }

  function updateGrid() {
    const all = getFiltered();
    const countEl = el.querySelector('.sport-picker-count');
    const grid = el.querySelector('.recipe-grid');
    grid.innerHTML = '';
    if (F.muscle === 'all') {
      countEl.textContent = '';
      grid.innerHTML = '<div class="uni-empty">Выберите группу мышц, чтобы подобрать упражнения</div>';
      return;
    }
    if (!all.length) {
      countEl.textContent = '';
      grid.innerHTML = '<div class="uni-empty">Ничего не найдено — ослабьте фильтры</div>';
      return;
    }
    countEl.textContent = all.length > RESULT_CAP
      ? `Показано ${RESULT_CAP} из ${all.length} — уточните фильтры`
      : `Найдено: ${all.length}`;
    for (const ex of all.slice(0, RESULT_CAP)) {
      const card = renderExerciseCard(ex);
      card.onclick = async () => {
        const { showExerciseModal } = await import('./sport-catalog-modal.js');
        showExerciseModal(async () => { await loadExerciseCatalog(); updateGrid(); }, ex);
      };
      grid.appendChild(card);
    }
  }

  function render() {
    el.querySelector('.sport-picker-steps').innerHTML = stepsHtml();
    updateGrid();
  }

  el.innerHTML = `<div class="recipe-pane sport-picker">
    <div class="sport-picker-steps"></div>
    <div class="sport-picker-count"></div>
    <div class="recipe-grid"></div></div>`;

  el.querySelector('.sport-picker-steps').addEventListener('click', (e) => {
    const chip = e.target.closest('.rf-chip');
    if (!chip) return;
    if (chip.dataset.equip != null) {
      const v = chip.dataset.equip;
      if (F.equipment.has(v)) F.equipment.delete(v); else F.equipment.add(v);
      render();
      return;
    }
    const g = chip.dataset.group;
    if (!g) return;
    F[g] = chip.dataset.val;
    if (g === 'equipMode' && chip.dataset.val !== 'with') F.equipment.clear();
    render();
  });

  render();
}
