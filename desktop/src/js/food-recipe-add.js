// ── food-recipe-add.js — Add recipe modal ──
import { invoke } from './state.js';

const CUISINES = { kz: '🇰🇿 Казахская', ru: '🇷🇺 Русская', it: '🇮🇹 Итальянская', jp: '🇯🇵 Японская', ge: '🇬🇪 Грузинская', tr: '🇹🇷 Турецкая', other: '🌍 Другая' };

export function showAddRecipeModal(reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новый рецепт</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="r-name"></div>
    <div class="form-group"><label class="form-label">Описание</label><input class="form-input" id="r-desc"></div>
    <div class="form-group"><label class="form-label">Ингредиенты (каждый с новой строки: название:100г)</label><textarea class="form-input" id="r-ingr" rows="4" placeholder="гречка:150г\nкурица:200г\nлук:1шт"></textarea></div>
    <div class="form-group"><label class="form-label">Инструкция (каждый шаг с новой строки)</label><textarea class="form-input" id="r-instr" rows="4"></textarea></div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">Подготовка (мин)</label><input class="form-input" id="r-prep" type="number" value="10"></div>
      <div class="form-group"><label class="form-label">Готовка (мин)</label><input class="form-input" id="r-cook" type="number" value="20"></div>
      <div class="form-group"><label class="form-label">Порции</label><input class="form-input" id="r-serv" type="number" value="2"></div>
      <div class="form-group"><label class="form-label">Калории</label><input class="form-input" id="r-cal" type="number"></div>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">Тип</label>
        <select class="form-select" id="r-tags" style="width:100%;">
          <option value="breakfast">Завтрак</option><option value="lunch">Обед</option>
          <option value="dinner">Ужин</option><option value="universal">Универсальный</option>
        </select></div>
      <div class="form-group"><label class="form-label">Сложность</label>
        <select class="form-select" id="r-diff" style="width:100%;">
          <option value="easy">Лёгкий</option><option value="medium">Средний</option>
        </select></div>
      <div class="form-group"><label class="form-label">Кухня</label>
        <select class="form-select" id="r-cuisine" style="width:100%;">
          ${Object.entries(CUISINES).map(([k,v]) => `<option value="${k}">${v}</option>`).join('')}
        </select></div>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">Полезность (1-10)</label><input class="form-input" id="r-health" type="number" min="1" max="10" value="5"></div>
      <div class="form-group"><label class="form-label">Цена (1-10)</label><input class="form-input" id="r-price" type="number" min="1" max="10" value="5"></div>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">Белки (г)</label><input class="form-input" id="r-protein" type="number" value="0"></div>
      <div class="form-group"><label class="form-label">Жиры (г)</label><input class="form-input" id="r-fat" type="number" value="0"></div>
      <div class="form-group"><label class="form-label">Углеводы (г)</label><input class="form-input" id="r-carbs" type="number" value="0"></div>
    </div>
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
    const ingrText = document.getElementById('r-ingr')?.value?.trim() || '';
    const ingredientItems = parseIngredientLines(ingrText);
    const ingrFlat = ingredientItems.map(i => `${i.name}: ${i.amount}${i.unit}`).join(', ');
    try {
      await invoke('create_recipe', {
        name,
        description: document.getElementById('r-desc')?.value?.trim() || '',
        ingredients: ingrFlat,
        instructions: document.getElementById('r-instr')?.value?.trim() || '',
        prepTime: parseInt(document.getElementById('r-prep')?.value) || 0,
        cookTime: parseInt(document.getElementById('r-cook')?.value) || 0,
        servings: parseInt(document.getElementById('r-serv')?.value) || 1,
        calories: parseInt(document.getElementById('r-cal')?.value) || 0,
        tags: document.getElementById('r-tags')?.value || 'universal',
        difficulty: document.getElementById('r-diff')?.value || 'easy',
        cuisine: document.getElementById('r-cuisine')?.value || 'kz',
        healthScore: parseInt(document.getElementById('r-health')?.value) || 5,
        priceScore: parseInt(document.getElementById('r-price')?.value) || 5,
        protein: parseInt(document.getElementById('r-protein')?.value) || 0,
        fat: parseInt(document.getElementById('r-fat')?.value) || 0,
        carbs: parseInt(document.getElementById('r-carbs')?.value) || 0,
        ingredientItems,
      });
      overlay.remove();
      if (reloadFn) await reloadFn();
    } catch (e) { alert('Error: ' + e); }
  });
}

function parseIngredientLines(text) {
  return text.split('\n').map(l => l.trim()).filter(Boolean).map(line => {
    const ci = line.indexOf(':');
    if (ci === -1) return { name: line, amount: 0, unit: '' };
    const name = line.slice(0, ci).trim(), rest = line.slice(ci + 1).trim();
    const m = rest.match(/^(\d+\.?\d*)\s*(.*)$/);
    return m ? { name, amount: parseFloat(m[1]), unit: m[2] || 'г' } : { name, amount: 0, unit: rest };
  });
}
