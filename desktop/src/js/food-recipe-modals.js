// ── food-recipe-modals.js — Recipe detail & add modals ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export async function showRecipeDetail(id, reloadFn) {
  let recipe;
  try { recipe = await invoke('get_recipe', { id }); } catch (e) { alert('Error: ' + e); return; }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const totalTime = (recipe.prep_time || 0) + (recipe.cook_time || 0);
  const items = recipe.ingredient_items || [];
  const instructions = (recipe.instructions || '').split('\n').filter(Boolean);
  const diffLabel = recipe.difficulty === 'medium' ? 'Средний' : 'Лёгкий';

  function fmtIngr(i) {
    const amt = i.amount ? `${i.amount}${escapeHtml(i.unit)}` : '';
    return `<span class="ingr-item"><span class="ingr-tag" data-ingr="${escapeHtml(i.name)}">${escapeHtml(i.name)}</span>${amt ? `<span class="ingr-amt">${amt}</span>` : ''}</span>`;
  }
  const ingrHtml = items.length > 0
    ? items.map(fmtIngr).join('')
    : (recipe.ingredients || '').split(',').map(s => s.trim()).filter(Boolean).map(n => `<span class="ingr-tag">${escapeHtml(n)}</span>`).join('');

  overlay.innerHTML = `<div class="modal recipe-detail-modal">
    <div class="modal-title">${escapeHtml(recipe.name)}</div>
    ${recipe.description ? `<p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px;">${escapeHtml(recipe.description)}</p>` : ''}
    <div class="recipe-detail-meta">
      ${totalTime ? `<span class="badge badge-blue">\u23f1 ${totalTime} \u043c\u0438\u043d</span>` : ''}
      <span class="badge badge-gray">\ud83d\udc65 ${recipe.servings || 1} \u043f\u043e\u0440\u0446.</span>
      <span class="badge badge-green">${recipe.calories || '\u2014'} kcal</span>
      <span class="badge badge-purple">${diffLabel}</span>
    </div>
    <div class="recipe-detail-section">
      <h4>\u0418\u043d\u0433\u0440\u0435\u0434\u0438\u0435\u043d\u0442\u044b</h4>
      <div class="recipe-ingr-tags">${ingrHtml}</div>
    </div>
    <div class="recipe-detail-section">
      <h4>\u041f\u0440\u0438\u0433\u043e\u0442\u043e\u0432\u043b\u0435\u043d\u0438\u0435</h4>
      <ol class="recipe-instructions">${instructions.map(s => `<li>${escapeHtml(s.replace(/^\d+\.\s*/, ''))}</li>`).join('')}</ol>
    </div>
    <div class="modal-actions">
      <button class="btn-danger" id="recipe-del">\u0423\u0434\u0430\u043b\u0438\u0442\u044c</button>
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">\u0417\u0430\u043a\u0440\u044b\u0442\u044c</button>
    </div>
  </div>`;

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  document.getElementById('recipe-del')?.addEventListener('click', async () => {
    if (!confirm('\u0423\u0434\u0430\u043b\u0438\u0442\u044c \u0440\u0435\u0446\u0435\u043f\u0442?')) return;
    try { await invoke('delete_recipe', { id }); overlay.remove(); if (reloadFn) await reloadFn(); } catch (e) { alert('Error: ' + e); }
  });
}

export function showAddRecipeModal(reloadFn) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">\u041d\u043e\u0432\u044b\u0439 \u0440\u0435\u0446\u0435\u043f\u0442</div>
    <div class="form-group"><label class="form-label">\u041d\u0430\u0437\u0432\u0430\u043d\u0438\u0435</label><input class="form-input" id="r-name"></div>
    <div class="form-group"><label class="form-label">\u041e\u043f\u0438\u0441\u0430\u043d\u0438\u0435</label><input class="form-input" id="r-desc"></div>
    <div class="form-group"><label class="form-label">\u0418\u043d\u0433\u0440\u0435\u0434\u0438\u0435\u043d\u0442\u044b (\u043a\u0430\u0436\u0434\u044b\u0439 \u0441 \u043d\u043e\u0432\u043e\u0439 \u0441\u0442\u0440\u043e\u043a\u0438: \u043d\u0430\u0437\u0432\u0430\u043d\u0438\u0435:100\u0433)</label><textarea class="form-input" id="r-ingr" rows="4" placeholder="\u0433\u0440\u0435\u0447\u043a\u0430:150\u0433\n\u043a\u0443\u0440\u0438\u0446\u0430:200\u0433\n\u043b\u0443\u043a:1\u0448\u0442"></textarea></div>
    <div class="form-group"><label class="form-label">\u0418\u043d\u0441\u0442\u0440\u0443\u043a\u0446\u0438\u044f (\u043a\u0430\u0436\u0434\u044b\u0439 \u0448\u0430\u0433 \u0441 \u043d\u043e\u0432\u043e\u0439 \u0441\u0442\u0440\u043e\u043a\u0438)</label><textarea class="form-input" id="r-instr" rows="4"></textarea></div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">\u041f\u043e\u0434\u0433\u043e\u0442\u043e\u0432\u043a\u0430 (\u043c\u0438\u043d)</label><input class="form-input" id="r-prep" type="number" value="10"></div>
      <div class="form-group"><label class="form-label">\u0413\u043e\u0442\u043e\u0432\u043a\u0430 (\u043c\u0438\u043d)</label><input class="form-input" id="r-cook" type="number" value="20"></div>
      <div class="form-group"><label class="form-label">\u041f\u043e\u0440\u0446\u0438\u0438</label><input class="form-input" id="r-serv" type="number" value="2"></div>
      <div class="form-group"><label class="form-label">\u041a\u0430\u043b\u043e\u0440\u0438\u0438</label><input class="form-input" id="r-cal" type="number"></div>
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
      <div class="form-group"><label class="form-label">\u0422\u0438\u043f</label>
        <select class="form-select" id="r-tags" style="width:100%;">
          <option value="breakfast">\u0417\u0430\u0432\u0442\u0440\u0430\u043a</option><option value="lunch">\u041e\u0431\u0435\u0434</option>
          <option value="dinner">\u0423\u0436\u0438\u043d</option><option value="universal">\u0423\u043d\u0438\u0432\u0435\u0440\u0441\u0430\u043b\u044c\u043d\u044b\u0439</option>
        </select></div>
      <div class="form-group"><label class="form-label">\u0421\u043b\u043e\u0436\u043d\u043e\u0441\u0442\u044c</label>
        <select class="form-select" id="r-diff" style="width:100%;">
          <option value="easy">\u041b\u0451\u0433\u043a\u0438\u0439</option><option value="medium">\u0421\u0440\u0435\u0434\u043d\u0438\u0439</option>
        </select></div>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">\u041e\u0442\u043c\u0435\u043d\u0430</button>
      <button class="btn-primary" id="r-save">\u0421\u043e\u0445\u0440\u0430\u043d\u0438\u0442\u044c</button>
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
        ingredientItems,
      });
      overlay.remove();
      if (reloadFn) await reloadFn();
    } catch (e) { alert('Error: ' + e); }
  });
}

function parseIngredientLines(text) {
  return text.split('\n').map(line => {
    line = line.trim();
    if (!line) return null;
    const colonIdx = line.indexOf(':');
    if (colonIdx === -1) return { name: line, amount: 0, unit: '' };
    const name = line.slice(0, colonIdx).trim();
    const rest = line.slice(colonIdx + 1).trim();
    const match = rest.match(/^(\d+\.?\d*)\s*(.*)$/);
    if (match) return { name, amount: parseFloat(match[1]), unit: match[2] || 'г' };
    return { name, amount: 0, unit: rest };
  }).filter(Boolean);
}
