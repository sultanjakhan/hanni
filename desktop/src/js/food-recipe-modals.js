// ── food-recipe-modals.js — Recipe detail modal ──
import { invoke } from './state.js';
import { escapeHtml, ingrCat, confirmModal } from './utils.js';
import { loadCuisines } from './food-recipe-filters.js';

export async function showRecipeDetail(id, reloadFn) {
  let recipe;
  try { recipe = await invoke('get_recipe', { id }); } catch (e) { alert('Error: ' + e); return; }

  const baseServ = recipe.servings || 1;
  let curServ = baseServ;
  const items = recipe.ingredient_items || [];
  const baseCal = recipe.calories || 0;
  const totalTime = (recipe.prep_time || 0) + (recipe.cook_time || 0);
  const instructions = (recipe.instructions || '').split('\n').filter(Boolean);
  const diffLabel = { easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' }[recipe.difficulty] || 'Лёгкий';
  const cuisines = await loadCuisines();
  const cuisineLabel = cuisines.find(c => c.id === recipe.cuisine)?.label || '🌍 Другая';
  let isFav = recipe.favorite === 1;
  const p = recipe.protein || 0, f = recipe.fat || 0, c = recipe.carbs || 0;
  const bjuHtml = (p || f || c) ? `<span class="badge badge-gray">Б${p} Ж${f} У${c}</span>` : '';

  function fmtIngr(i) {
    const cat = ingrCat(i.name);
    const catCls = cat ? ` ingr-cat-${cat}` : '';
    const amt = i.amount ? `${i.amount}${escapeHtml(i.unit)}` : '';
    return `<span class="ingr-item"><span class="ingr-tag${catCls}" data-ingr="${escapeHtml(i.name)}">${escapeHtml(i.name)}</span>${amt ? `<span class="ingr-amt" data-base="${i.amount}" data-unit="${escapeHtml(i.unit)}">${amt}</span>` : ''}</span>`;
  }
  const ingrHtml = items.length > 0
    ? items.map(fmtIngr).join('')
    : (recipe.ingredients || '').split(',').map(s => s.trim()).filter(Boolean).map(n => { const cat = ingrCat(n); const cls = cat ? ` ingr-cat-${cat}` : ''; return `<span class="ingr-tag${cls}">${escapeHtml(n)}</span>`; }).join('');

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal recipe-detail-modal">
    <div class="modal-title-row">
      <div class="modal-title">${escapeHtml(recipe.name)}</div>
      <button class="fav-btn${isFav ? ' active' : ''}" id="fav-btn" title="Избранное">★</button>
    </div>
    ${recipe.description ? `<p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px;">${escapeHtml(recipe.description)}</p>` : ''}
    <div class="recipe-detail-meta">
      ${totalTime ? `<span class="badge badge-blue">⏱ ${totalTime} мин</span>` : ''}
      <span class="badge badge-gray servings-ctrl">
        <button class="serv-btn" data-dir="-1">−</button>
        <span id="serv-label">👥 ${curServ} порц.</span>
        <button class="serv-btn" data-dir="1">+</button>
      </span>
      <span class="badge badge-green" id="cal-label">${baseCal || '—'} kcal</span>
      ${bjuHtml}
      <span class="badge badge-purple">${diffLabel}</span>
      <span class="badge badge-gray">${cuisineLabel}</span>
      <span class="badge badge-green">❤ ${recipe.health_score || 5}/10</span>
      <span class="badge badge-yellow">💰 ${recipe.price_score || 5}/10</span>
    </div>
    <div class="recipe-detail-section">
      <h4>Ингредиенты</h4>
      <div class="recipe-ingr-tags">${ingrHtml}</div>
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

  function updateAmounts() {
    const ratio = curServ / baseServ;
    overlay.querySelector('#serv-label').textContent = `👥 ${curServ} порц.`;
    overlay.querySelector('#cal-label').textContent = (baseCal ? Math.round(baseCal * ratio) : '—') + ' kcal';
    overlay.querySelectorAll('.ingr-amt').forEach(el => {
      const base = parseFloat(el.dataset.base);
      if (!base) return;
      el.textContent = `${Math.round(base * ratio * 10) / 10}${el.dataset.unit}`;
    });
  }

  overlay.querySelectorAll('.serv-btn').forEach(btn => btn.addEventListener('click', (e) => {
    e.stopPropagation();
    const next = curServ + parseInt(btn.dataset.dir);
    if (next >= 1 && next <= 20) { curServ = next; updateAmounts(); }
  }));
  overlay.querySelector('#fav-btn')?.addEventListener('click', async () => {
    try { const v = await invoke('toggle_favorite_recipe', { id }); isFav = v === 1; overlay.querySelector('#fav-btn').classList.toggle('active', isFav); } catch {}
  });
  overlay.querySelector('#recipe-del')?.addEventListener('click', async () => {
    if (!await confirmModal('Удалить рецепт?', 'Удалить')) return;
    try { await invoke('delete_recipe', { id }); overlay.remove(); if (reloadFn) await reloadFn(); } catch (e) { alert('Error: ' + e); }
  });

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
}

export { showAddRecipeModal } from './food-recipe-add.js';
