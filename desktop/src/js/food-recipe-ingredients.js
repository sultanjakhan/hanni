// ── food-recipe-ingredients.js — Structured ingredient input with autocomplete ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, invalidateCatalogCache } from './food-recipe-filters.js';

const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];

export function renderIngredientRows(container, catalog) {
  container.innerHTML = '';
  addRow(container, catalog);
  const addBtn = document.createElement('button');
  addBtn.className = 'btn-secondary';
  addBtn.textContent = '+ Ингредиент';
  addBtn.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
  addBtn.onclick = () => { container.insertBefore(createRow(catalog), addBtn); };
  container.appendChild(addBtn);
}

function addRow(container, catalog) {
  const addBtn = container.querySelector('.btn-secondary');
  container.insertBefore(createRow(catalog), addBtn || null);
}

function createRow(catalog) {
  const row = document.createElement('div');
  row.className = 'ingr-row';
  const selUnit = UNITS[0];
  row.innerHTML = `<div class="ingr-row-main">
    <input class="form-input ingr-name-input" placeholder="Ингредиент..." autocomplete="off">
    <input class="form-input ingr-amount-input" type="number" placeholder="100">
    <div class="ingr-unit-acc"><button type="button" class="ingr-unit-btn">${selUnit} ▾</button>
      <div class="ingr-unit-dropdown" style="display:none">${UNITS.map(u => `<div class="ingr-unit-opt${u === selUnit ? ' active' : ''}" data-val="${u}">${u}</div>`).join('')}</div>
    </div>
    <button type="button" class="ingr-del-btn">&times;</button></div>`;
  setupUnitDD(row);
  row.querySelector('.ingr-del-btn').onclick = () => {
    if (row.parentElement.querySelectorAll('.ingr-row').length > 1) row.remove();
  };
  const nameInput = row.querySelector('.ingr-name-input');
  nameInput.addEventListener('input', () => showAutocomplete(row, nameInput, catalog));
  nameInput.addEventListener('blur', () => setTimeout(() => closeAC(row), 150));
  return row;
}

function setupUnitDD(row) {
  const btn = row.querySelector('.ingr-unit-btn'), dd = row.querySelector('.ingr-unit-dropdown');
  btn.onclick = (e) => { e.preventDefault(); dd.style.display = dd.style.display === 'none' ? '' : 'none'; };
  dd.querySelectorAll('.ingr-unit-opt').forEach(opt => {
    opt.onclick = () => {
      dd.querySelectorAll('.ingr-unit-opt').forEach(o => o.classList.remove('active'));
      opt.classList.add('active'); btn.textContent = `${opt.dataset.val} ▾`; dd.style.display = 'none';
    };
  });
  row.addEventListener('focusout', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
}

function showAutocomplete(row, input, catalog) {
  closeAC(row);
  const q = input.value.trim().toLowerCase();
  if (!q) return;
  const matches = catalog.filter(c => c.name.toLowerCase().includes(q)).slice(0, 12);
  const dd = document.createElement('div');
  dd.className = 'ingr-autocomplete';
  // Group matches by category
  const grouped = {};
  for (const item of matches) { (grouped[item.category] ||= []).push(item); }
  for (const cat of CAT_ORDER) {
    if (!grouped[cat]) continue;
    const hdr = document.createElement('div');
    hdr.className = 'ingr-ac-cat';
    hdr.textContent = CAT_LABELS[cat] || cat;
    dd.appendChild(hdr);
    for (const item of grouped[cat]) {
      const opt = document.createElement('div');
      opt.className = 'ingr-autocomplete-item';
      opt.textContent = item.name;
      opt.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row); };
      dd.appendChild(opt);
    }
  }
  if (!matches.some(m => m.name.toLowerCase() === q)) {
    const create = document.createElement('div');
    create.className = 'ingr-autocomplete-item ingr-autocomplete-create';
    create.textContent = `+ Создать «${input.value.trim()}»`;
    create.onmousedown = (e) => { e.preventDefault(); showCatPicker(row, input, catalog); };
    dd.appendChild(create);
  }
  if (dd.children.length) row.querySelector('.ingr-row-main').appendChild(dd);
}

function showCatPicker(row, input, catalog) {
  closeAC(row);
  const name = input.value.trim();
  const dd = document.createElement('div');
  dd.className = 'ingr-autocomplete';
  const hdr = document.createElement('div');
  hdr.className = 'ingr-ac-cat';
  hdr.textContent = `Категория для «${name}»:`;
  dd.appendChild(hdr);
  for (const cat of CAT_ORDER) {
    const opt = document.createElement('div');
    opt.className = 'ingr-autocomplete-item';
    opt.textContent = CAT_LABELS[cat] || cat;
    opt.onmousedown = async (e) => {
      e.preventDefault();
      try { await invoke('add_ingredient_to_catalog', { name, category: cat }); catalog.push({ name, category: cat }); invalidateCatalogCache(); } catch {}
      selectItem(input, name, row);
    };
    dd.appendChild(opt);
  }
  row.querySelector('.ingr-row-main').appendChild(dd);
  input.addEventListener('blur', () => setTimeout(() => closeAC(row), 200), { once: true });
}

function selectItem(input, name, row) {
  input.value = name; closeAC(row);
  row.querySelector('.ingr-amount-input')?.focus();
}
function closeAC(row) { row.querySelector('.ingr-autocomplete')?.remove(); }

export function collectIngredientItems(container) {
  const items = [];
  container.querySelectorAll('.ingr-row').forEach(row => {
    const name = row.querySelector('.ingr-name-input')?.value?.trim();
    if (!name) return;
    const amount = parseFloat(row.querySelector('.ingr-amount-input')?.value) || 0;
    const unit = row.querySelector('.ingr-unit-opt.active')?.dataset.val || 'г';
    items.push({ name, amount, unit });
  });
  return items;
}
