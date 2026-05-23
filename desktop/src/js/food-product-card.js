// ── food-product-card.js — Product card for catalog grid ──
import { CAT_LABELS } from './food-recipe-filters.js';

const CAT_COLORS = {
  meat: 'red', fish: 'blue', veg: 'green', fruit: 'orange', grain: 'yellow',
  dairy: 'purple', legumes: 'teal', nuts: 'brown', spice: 'pink',
  oil: 'amber', bakery: 'warm', drinks: 'cyan', other: 'gray',
};

export function renderProductCard(product, opts = {}) {
  const { productLevel = '', blockedTags = new Set(), blockedCategory = false } = opts;
  const div = document.createElement('div');
  div.className = 'product-card'
    + (productLevel === 'hard' ? ' product-card--blocked'
      : productLevel === 'soft' ? ' product-card--soft'
      : productLevel === 'love' ? ' product-card--love' : '');
  div.dataset.id = product.id;
  div.dataset.blType = 'product';
  div.dataset.blValue = product.name;
  div.dataset.blId = product.id;
  const color = CAT_COLORS[product.category] || 'gray';
  const label = CAT_LABELS[product.category] || product.category;
  const catCls = blockedCategory ? ' product-card-tag--blocked' : '';
  const tagsHtml = (product.tags || '').split(',').filter(Boolean)
    .map(t => {
      const tag = t.trim();
      const cls = blockedTags.has(tag.toLowerCase()) ? ' product-card-tag--blocked' : '';
      return `<span class="product-card-tag${cls}">${esc(tag)}</span>`;
    }).join('');
  const blockIcon = productLevel === 'hard' ? '<span class="product-card-blocked-icon" title="Не ем">🚫</span>'
    : productLevel === 'soft' ? '<span class="product-card-blocked-icon" title="Не люблю">👎</span>'
    : productLevel === 'love' ? '<span class="product-card-blocked-icon" title="Люблю">💚</span>' : '';
  div.innerHTML = `
    <div class="product-card-name">${blockIcon}${esc(product.name)}</div>
    <div class="product-card-tags"><span class="product-card-cat product-cat-${color}${catCls}">${label}</span>${tagsHtml}</div>
    <button class="bl-quick" title="В блэклист">⊘</button>`;
  return div;
}

function esc(s) { return s.replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
