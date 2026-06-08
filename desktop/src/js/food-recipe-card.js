// ── food-recipe-card.js — Recipe card rendering ──
import { escapeHtml } from './utils.js';
import { catalogCat } from './food-recipe-filters.js';

const MEAL_COLORS = { breakfast: 'green', lunch: 'yellow', dinner: 'red', universal: 'blue' };

// "YYYY-MM-DD" → human "N дн. назад" relative to today.
function cookedAgo(dateStr) {
  const then = new Date(dateStr + 'T12:00:00');
  if (isNaN(then.getTime())) return '';
  const days = Math.floor((Date.now() - then.getTime()) / 86400000);
  if (days <= 0) return 'сегодня';
  if (days === 1) return 'вчера';
  return `${days} дн. назад`;
}

export const getIngrNames = (r) => (r.ingredients || '').split(',').map(s => {
  const i = s.indexOf(':'); return (i > -1 ? s.slice(0, i) : s).trim();
}).filter(Boolean);

export function renderCard(r, onIngrClick, onDuplicate) {
  const tags = (r.tags || '').split(/[,\s]+/).map(t => t.trim()).filter(Boolean)
    .filter(t => !t.startsWith('shared-by:'));
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

  const fridge = (r._missing == null) ? ''
    : (r._missing === 0 ? '<span class="badge badge-green">✓ есть всё</span>'
       : `<span class="badge badge-gray">не хватает ${r._missing}</span>`);
  const div = document.createElement('div');
  div.className = 'recipe-card' + (r.favorite === 1 ? ' recipe-fav' : '');
  div.dataset.id = r.id;
  const dupBtn = onDuplicate
    ? `<button class="recipe-card-dup-btn" title="Дублировать рецепт" aria-label="Дублировать">⧉</button>`
    : '';
  div.innerHTML = `
    ${r.image ? `<img class="recipe-card-thumb" src="${escapeHtml(r.image)}" alt="">` : ''}
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
    <div class="recipe-card-cooked${r.last_cooked ? '' : ' is-never'}">${
      r.last_cooked ? `🍳 готовили ${cookedAgo(r.last_cooked)}` : 'ещё не готовили'}</div>
    <div class="recipe-card-tags">${fridge}${badgesHtml}</div>
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
