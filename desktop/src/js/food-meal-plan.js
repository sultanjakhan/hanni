// ── food-meal-plan.js — Meal plan for calendar day view ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const MEAL_LABELS = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', snack: 'Перекус' };
const MEAL_COLORS = { breakfast: 'var(--accent-yellow, #d4a843)', lunch: 'var(--color-green)', dinner: 'var(--accent-purple)', snack: 'var(--text-muted)' };

export async function renderMealPlanBlock(date) {
  const meals = await invoke('get_meal_plan', { date }).catch(() => []);
  if (!meals.length) return '';

  const items = meals.map(m => {
    const label = MEAL_LABELS[m.meal_type] || m.meal_type;
    const color = MEAL_COLORS[m.meal_type] || 'var(--text-secondary)';
    return `<div class="meal-plan-item" data-meal-id="${m.id}">
      <span class="meal-plan-type" style="color:${color};">${label}</span>
      <span class="meal-plan-name">${escapeHtml(m.recipe_name)}</span>
      <span class="meal-plan-cal">${m.calories || '—'} kcal</span>
      <button class="meal-plan-del" data-del-id="${m.id}" title="Убрать">&times;</button>
    </div>`;
  }).join('');

  const totalCal = meals.reduce((s, m) => s + (m.calories || 0), 0);
  return `<div class="meal-plan-block">
    <div class="meal-plan-header">
      <span>🍽 План питания</span>
      <span class="meal-plan-total">${totalCal} kcal</span>
    </div>
    ${items}
  </div>`;
}

export function showMealPlanModal(date, reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить приём пищи — ${date}</div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="mp-type" style="width:100%;">
        <option value="breakfast">Завтрак</option><option value="lunch">Обед</option>
        <option value="dinner">Ужин</option><option value="snack">Перекус</option>
      </select></div>
    <div class="form-group"><label class="form-label">Рецепт</label>
      <input class="form-input" id="mp-search" placeholder="Поиск...">
      <div id="mp-list" class="mp-recipe-list" style="max-height:200px;overflow-y:auto;margin-top:6px;"></div>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  let allRecipes = [];
  (async () => {
    allRecipes = await invoke('get_recipes', { search: null }).catch(() => []);
    renderList('');
  })();

  const searchInput = document.getElementById('mp-search');
  if (searchInput) searchInput.oninput = () => renderList(searchInput.value.trim().toLowerCase());

  function renderList(q) {
    const list = document.getElementById('mp-list');
    if (!list) return;
    const filtered = q ? allRecipes.filter(r => r.name.toLowerCase().includes(q)) : allRecipes;
    list.innerHTML = filtered.map(r =>
      `<div class="mp-recipe-option" data-rid="${r.id}">
        <span>${escapeHtml(r.name)}</span>
        <span style="color:var(--text-muted);font-size:12px;">${r.calories || '—'} kcal</span>
      </div>`
    ).join('') || '<div style="padding:8px;color:var(--text-muted);font-size:13px;">Нет рецептов</div>';

    list.querySelectorAll('.mp-recipe-option').forEach(opt => {
      opt.onclick = async () => {
        const recipeId = parseInt(opt.dataset.rid);
        const mealType = document.getElementById('mp-type')?.value || 'lunch';
        try {
          await invoke('plan_meal', { date, mealType, recipeId, notes: null });
          overlay.remove();
          if (reloadFn) reloadFn();
        } catch (e) { alert('Error: ' + e); }
      };
    });
  }
}
