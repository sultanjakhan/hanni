// ── food-product-card.js — Product card for catalog grid ──
import { CAT_LABELS } from './food-recipe-filters.js';

const CAT_COLORS = {
  meat: 'red', fish: 'blue', veg: 'green', fruit: 'orange', grain: 'yellow',
  dairy: 'purple', legumes: 'teal', nuts: 'brown', spice: 'pink',
  oil: 'amber', bakery: 'warm', drinks: 'cyan', other: 'gray',
};

export function renderProductCard(product) {
  const div = document.createElement('div');
  div.className = 'product-card';
  div.dataset.id = product.id;
  const color = CAT_COLORS[product.category] || 'gray';
  const label = CAT_LABELS[product.category] || product.category;
  const tagsHtml = (product.tags || '').split(',').filter(Boolean)
    .map(t => `<span class="product-card-tag">${esc(t.trim())}</span>`).join('');
  div.innerHTML = `
    <div class="product-card-name">${esc(product.name)}</div>
    <div class="product-card-tags"><span class="product-card-cat product-cat-${color}">${label}</span>${tagsHtml}</div>`;
  return div;
}

function esc(s) { return s.replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
