// ── food-recipe-card.js — Recipe card rendering ──
import { escapeHtml } from './utils.js';
import { catalogCat } from './food-recipe-filters.js';

const MEAL_COLORS = { breakfast: 'green', lunch: 'yellow', dinner: 'red', universal: 'blue' };

export const getIngrNames = (r) => (r.ingredients || '').split(',').map(s => {
  const i = s.indexOf(':'); return (i > -1 ? s.slice(0, i) : s).trim();
}).filter(Boolean);

export function renderCard(r, onIngrClick, onDuplicate) {
  const tags = (r.tags || '').split(',').map(t => t.trim()).filter(Boolean);
  const badgesHtml = tags.map(t => {
    const c = MEAL_COLORS[t] || 'gray';
    const label = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', universal: 'Универсал' }[t] || t;
    return `<span class="badge badge-${c}">${label}</span>`;
  }).join('');
  const totalTime = (r.prep_time || 0) + (r.cook_time || 0);
  const diffLabel = { easy: 'Лёгкий', medium: 'Средний', hard: 'Сложный' }[r.difficulty] || 'Лёгкий';
  const ingrNames = getIngrNames(r);
  const ingrHtml = ingrNames.slice(0, 5).map(n => {
    const cat = catalogCat(n); const cls = cat ? ` ingr-cat-${cat}` : '';
    return `<span class="ingr-tag${cls}" data-ingr="${escapeHtml(n)}">${escapeHtml(n)}</span>`;
  }).join('') + (ingrNames.length > 5 ? `<span class="ingr-tag ingr-more">+${ingrNames.length - 5}</span>` : '');

  const div = document.createElement('div');
  div.className = 'recipe-card' + (r.favorite === 1 ? ' recipe-fav' : '');
  div.dataset.id = r.id;
  const dupBtn = onDuplicate
    ? `<button class="recipe-card-dup-btn" title="Дублировать рецепт" aria-label="Дублировать">⧉</button>`
    : '';
  div.innerHTML = `
    <div class="recipe-card-header">
      <span class="recipe-card-name">${r.favorite === 1 ? '★ ' : ''}${escapeHtml(r.name)}</span>
      <span class="recipe-card-cal">${r.calories || '—'} kcal</span>
      ${dupBtn}
    </div>
    <div class="recipe-card-meta">
      ${totalTime ? `<span>⏱ ${totalTime} мин</span>` : ''}
      <span>👥 ${r.servings || 1}</span>
      <span class="recipe-diff recipe-diff-${r.difficulty || 'easy'}">${diffLabel}</span>
      <span>❤${r.health_score || 5}</span><span>💰${r.price_score || 5}</span>
    </div>
    <div class="recipe-card-tags">${badgesHtml}</div>
    <div class="recipe-card-ingr">${ingrHtml}</div>`;
  div.querySelectorAll('.ingr-tag[data-ingr]').forEach(tag => {
    tag.addEventListener('click', (e) => { e.stopPropagation(); onIngrClick(tag.dataset.ingr); });
  });
  if (onDuplicate) {
    div.querySelector('.recipe-card-dup-btn')?.addEventListener('click', (e) => {
      e.stopPropagation(); onDuplicate(r.id);
    });
  }
  return div;
}
