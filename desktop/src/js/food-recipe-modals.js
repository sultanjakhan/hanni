// ── food-recipe-modals.js — Recipe detail modal ──
import { invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { loadCuisines, catalogCat, CAT_LABELS } from './food-recipe-filters.js';

// instructions holds either a JSON array of {text,min,ingredients} (new) or
// legacy newline-separated text. Normalise both to a step array.
function parseSteps(raw) {
  const s = (raw || '').trim();
  if (s.startsWith('[')) {
    try {
      const arr = JSON.parse(s);
      if (Array.isArray(arr)) return arr
        .map(x => ({ text: String(x.text || ''), min: x.min || 0, ingredients: Array.isArray(x.ingredients) ? x.ingredients : [] }))
        .filter(x => x.text);
    } catch { /* fall through to legacy */ }
  }
  return s.split('\n').map(l => l.trim()).filter(Boolean)
    .map(l => ({ text: l.replace(/^\d+\.\s*/, ''), min: 0, ingredients: [] }));
}

function stepsHtml(steps) {
  if (!steps.length) return '<div class="recipe-empty">Шаги не указаны</div>';
  return `<div class="recipe-steps">${steps.map((st, i) => {
    const time = st.min ? `<span class="badge badge-blue step-time-badge">⏱ ${st.min} мин</span>` : '';
    const prods = st.ingredients.length
      ? `<div class="step-prod-list">${st.ingredients.map(n => {
          const cat = catalogCat(n); const cls = cat ? ` ingr-cat-${cat}` : '';
          return `<span class="ingr-tag${cls}">${escapeHtml(n)}</span>`;
        }).join('')}</div>` : '';
    return `<div class="step-view"><div class="step-num">${i + 1}</div>
      <div class="step-view-body"><div class="step-view-text">${escapeHtml(st.text)}${time}</div>${prods}</div></div>`;
  }).join('')}</div>`;
}

// Ingredients as a grouped list with amounts; empty state when none.
function ingredientsHtml(items, legacy) {
  if (items.length) {
    const groups = {};
    for (const i of items) { const cat = catalogCat(i.name) || 'other'; (groups[cat] = groups[cat] || []).push(i); }
    return Object.entries(groups).map(([cat, list]) =>
      `<div class="ingr-group"><div class="ingr-group-head">${escapeHtml(CAT_LABELS[cat] || cat)}</div>${list.map(i => {
        const amt = i.amount ? `<span class="ingr-amt" data-base="${i.amount}" data-unit="${escapeHtml(i.unit)}">${i.amount}${escapeHtml(i.unit)}</span>` : '';
        const alts = String(i.alternatives || '').split(',').map(s => s.trim()).filter(Boolean);
        const altsHtml = alts.length ? ` <span class="ingr-line-alts">/ ${alts.map(escapeHtml).join(' / ')}</span>` : '';
        return `<div class="ingr-line" data-ingr="${escapeHtml(i.name)}"><span class="ingr-line-name">${escapeHtml(i.name)}${altsHtml}</span>${amt}</div>`;
      }).join('')}</div>`).join('');
  }
  const names = (legacy || '').split(',').map(s => s.trim()).filter(Boolean);
  if (!names.length) return '<div class="recipe-empty">Ингредиенты не указаны</div>';
  return `<div class="ingr-group">${names.map(n => `<div class="ingr-line"><span class="ingr-line-name">${escapeHtml(n)}</span></div>`).join('')}</div>`;
}

export async function showRecipeDetail(id, reloadFn) {
  let recipe;
  try { recipe = await invoke('get_recipe', { id }); } catch (e) { alert('Error: ' + e); return; }
  const log = await invoke('get_cooking_log', { recipeId: id }).catch(() => []);

  const baseServ = recipe.servings || 1;
  let curServ = baseServ;
  const items = recipe.ingredient_items || [];
  const baseCal = recipe.calories || 0;
  const totalTime = (recipe.prep_time || 0) + (recipe.cook_time || 0);
  const steps = parseSteps(recipe.instructions || '');
  const diffLabel = { easy: 'лёгкая', medium: 'средняя', hard: 'сложная' }[recipe.difficulty] || 'лёгкая';
  const MEAL_LABELS = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', universal: 'Универсал' };
  // Tolerant split (handles legacy space-joined `shared-by:`) + drop
  // meta-tags from chip rendering — `shared-by:guest` is metadata, not UX.
  const mealHtml = (recipe.tags || '').split(/[,\s]+/).map(t => t.trim()).filter(Boolean)
    .filter(t => !t.startsWith('shared-by:'))
    .map(t => `<span class="rd-tag rd-tag--meal">${MEAL_LABELS[t] || t}</span>`).join('');
  const cuisines = await loadCuisines();
  const cuisineLabel = cuisines.find(c => c.id === recipe.cuisine)?.label || '🌍 Другая';
  let isFav = recipe.favorite === 1;
  const p = recipe.protein || 0, f = recipe.fat || 0, c = recipe.carbs || 0;
  const bjuHtml = (p || f || c) ? `<span class="rd-tag">Белки ${p}г · Жиры ${f}г · Углеводы ${c}г (на 100 г)</span>` : '';

  const ingrHtml = ingredientsHtml(items, recipe.ingredients);

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal recipe-detail-modal">
    <div class="modal-title-row">
      <div class="modal-title">${escapeHtml(recipe.name)}</div>
      <div class="rd-title-actions">
        <button class="rd-edit-btn" id="recipe-edit" title="Изменить">✎</button>
        <button class="fav-btn${isFav ? ' active' : ''}" id="fav-btn" title="Избранное">★</button>
      </div>
    </div>
    ${recipe.description ? `<p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px;">${escapeHtml(recipe.description)}</p>` : ''}
    ${recipe.image ? `<img class="rd-photo" src="${recipe.image}" alt="">` : ''}
    <div class="rd-stats">
      <div class="rd-stat"><div class="rd-stat-val">${totalTime || '—'}</div><div class="rd-stat-lbl">минут</div></div>
      <div class="rd-stat">
        <div class="rd-stat-val rd-serv"><button class="serv-btn" data-dir="-1">−</button><span id="serv-label">${curServ}</span><button class="serv-btn" data-dir="1">+</button></div>
        <div class="rd-stat-lbl">порций</div>
      </div>
      <div class="rd-stat"><div class="rd-stat-val">${baseCal || '—'}</div><div class="rd-stat-lbl">ккал/100г</div></div>
    </div>
    <div class="rd-tags">
      ${mealHtml}
      <span class="rd-tag">${cuisineLabel}</span>
      <span class="rd-tag">Сложность: ${diffLabel}</span>
      <span class="rd-tag">❤ Полезность ${recipe.health_score || 5}/10</span>
      <span class="rd-tag">💰 Бюджет ${recipe.price_score || 5}/10</span>
      ${bjuHtml}
    </div>
    <div class="recipe-detail-section">
      <h4>Ингредиенты</h4>
      <div class="recipe-ingr-list">${ingrHtml}</div>
    </div>
    <div class="recipe-detail-section">
      <h4>Приготовление</h4>
      ${stepsHtml(steps)}
    </div>
    <div class="recipe-detail-section">
      <h4>История приготовления</h4>
      ${log.length ? `<div class="cook-log">${log.map(e => `<div class="cook-log-entry">
        <div class="cook-log-row"><span class="cook-log-date">${escapeHtml(e.date)}</span>
          <span class="cook-log-stars">${[1, 2, 3, 4, 5].map(n => `<span class="rd-star${n <= e.taste_rating ? ' on' : ''}">★</span>`).join('')}</span></div>
        ${e.cook_note ? `<div class="cook-log-note">${escapeHtml(e.cook_note)}</div>` : ''}</div>`).join('')}</div>` : '<div class="recipe-empty">Ещё не готовили</div>'}
    </div>
    <div class="modal-actions">
      <button class="btn-danger" id="recipe-del">Удалить</button>
      <button class="btn-secondary" id="recipe-close">Закрыть</button>
    </div>
  </div>`;

  function updateAmounts() {
    const ratio = curServ / baseServ;
    overlay.querySelector('#serv-label').textContent = String(curServ);
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

  overlay.querySelector('#recipe-edit')?.addEventListener('click', async () => {
    overlay.remove();
    const { showEditRecipeModal } = await import('./food-recipe-add.js');
    await showEditRecipeModal(recipe, reloadFn);
  });

  overlay.querySelector('#recipe-close')?.addEventListener('click', () => overlay.remove());

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
}

export { showAddRecipeModal } from './food-recipe-add.js';
