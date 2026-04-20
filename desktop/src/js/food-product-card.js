// ── food-product-card.js — Product card for catalog grid ──
import { CAT_LABELS } from './food-recipe-filters.js';

const CAT_COLORS = {
  meat: 'red', fish: 'blue', veg: 'green', fruit: 'orange', grain: 'yellow',
  dairy: 'purple', legumes: 'teal', nuts: 'brown', spice: 'pink',
  oil: 'amber', bakery: 'warm', drinks: 'cyan', other: 'gray',
};

export function renderProductCard(product, opts = {}) {
  const { productBlocked = false, blockedTags = new Set(), blockedCategory = false } = opts;
  const div = document.createElement('div');
  div.className = 'product-card' + (productBlocked ? ' product-card--blocked' : '');
  div.dataset.id = product.id;
  const color = CAT_COLORS[product.category] || 'gray';
  const label = CAT_LABELS[product.category] || product.category;
  const catCls = blockedCategory ? ' product-card-tag--blocked' : '';
  const tagsHtml = (product.tags || '').split(',').filter(Boolean)
    .map(t => {
      const tag = t.trim();
      const cls = blockedTags.has(tag.toLowerCase()) ? ' product-card-tag--blocked' : '';
      return `<span class="product-card-tag${cls}">${esc(tag)}</span>`;
    }).join('');
  const blockIcon = productBlocked ? '<span class="product-card-blocked-icon" title="В блэклисте">🚫</span>' : '';
  div.innerHTML = `
    <div class="product-card-name">${blockIcon}${esc(product.name)}</div>
    <div class="product-card-tags"><span class="product-card-cat product-cat-${color}${catCls}">${label}</span>${tagsHtml}</div>`;
  return div;
}

function esc(s) { return s.replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
