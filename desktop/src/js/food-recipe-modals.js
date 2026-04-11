// ── food-recipe-modals.js — Recipe detail & add modals ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export async function showRecipeDetail(id, reloadFn) {
  let recipe;
  try { recipe = await invoke('get_recipe', { id }); } catch (e) { alert('Error: ' + e); return; }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const totalTime = (recipe.prep_time || 0) + (recipe.cook_time || 0);
  const ingredients = (recipe.ingredients || '').split(',').map(s => s.trim()).filter(Boolean);
  const instructions = (recipe.instructions || '').split('\n').filter(Boolean);

  overlay.innerHTML = `<div class="modal recipe-detail-modal">
    <div class="modal-title">${escapeHtml(recipe.name)}</div>
    ${recipe.description ? `<p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px;">${escapeHtml(recipe.description)}</p>` : ''}
    <div class="recipe-detail-meta">
      ${totalTime ? `<span class="badge badge-blue">⏱ ${totalTime} мин</span>` : ''}
      <span class="badge badge-gray">👥 ${recipe.servings || 1} порц.</span>
      <span class="badge badge-green">${recipe.calories || '—'} kcal</span>
    </div>
    <div class="recipe-detail-section">
      <h4>Ингредиенты</h4>
      <ul class="recipe-ingredients">${ingredients.map(i => `<li>${escapeHtml(i)}</li>`).join('')}</ul>
    </div>
    <div class="recipe-detail-section">
      <h4>Приготовление</h4>
      <ol class="recipe-instructions">${instructions.map(s => `<li>${escapeHtml(s.replace(/^\d+\.\s*/, ''))}</li>`).join('')}</ol>
    </div>
    <div class="modal-actions">
      <button class="btn-danger" id="recipe-del">Удалить</button>
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Закрыть</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  document.getElementById('recipe-del')?.addEventListener('click', async () => {
    if (!confirm('Удалить рецепт?')) return;
    try { await invoke('delete_recipe', { id }); overlay.remove(); if (reloadFn) await reloadFn(); } catch (e) { alert('Error: ' + e); }
  });
}

export function showAddRecipeModal(reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новый рецепт</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="r-name"></div>
    <div class="form-group"><label class="form-label">Описание</label><input class="form-input" id="r-desc"></div>
    <div class="form-group"><label class="form-label">Ингредиенты (через запятую)</label><textarea class="form-input" id="r-ingr" rows="3"></textarea></div>
    <div class="form-group"><label class="form-label">Инструкция (каждый шаг с новой строки)</label><textarea class="form-input" id="r-instr" rows="4"></textarea></div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">Подготовка (мин)</label><input class="form-input" id="r-prep" type="number" value="10"></div>
      <div class="form-group"><label class="form-label">Готовка (мин)</label><input class="form-input" id="r-cook" type="number" value="20"></div>
      <div class="form-group"><label class="form-label">Порции</label><input class="form-input" id="r-serv" type="number" value="2"></div>
      <div class="form-group"><label class="form-label">Калории</label><input class="form-input" id="r-cal" type="number"></div>
    </div>
    <div class="form-group"><label class="form-label">Тип</label>
      <select class="form-select" id="r-tags" style="width:100%;">
        <option value="breakfast">Завтрак</option><option value="lunch">Обед</option>
        <option value="dinner">Ужин</option><option value="universal">Универсальный</option>
      </select></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="r-save">Сохранить</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  document.getElementById('r-save')?.addEventListener('click', async () => {
    const name = document.getElementById('r-name')?.value?.trim();
    if (!name) return;
    try {
      await invoke('create_recipe', {
        name,
        description: document.getElementById('r-desc')?.value?.trim() || '',
        ingredients: document.getElementById('r-ingr')?.value?.trim() || '',
        instructions: document.getElementById('r-instr')?.value?.trim() || '',
        prepTime: parseInt(document.getElementById('r-prep')?.value) || 0,
        cookTime: parseInt(document.getElementById('r-cook')?.value) || 0,
        servings: parseInt(document.getElementById('r-serv')?.value) || 1,
        calories: parseInt(document.getElementById('r-cal')?.value) || 0,
        tags: document.getElementById('r-tags')?.value || 'universal',
      });
      overlay.remove();
      if (reloadFn) await reloadFn();
    } catch (e) { alert('Error: ' + e); }
  });
}
