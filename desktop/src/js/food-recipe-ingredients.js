// ── food-recipe-ingredients.js — Structured ingredient input with autocomplete ──
import { invoke } from './state.js';
import { ingrCat } from './utils.js';

const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];

export function renderIngredientRows(container, catalog) {
  container.innerHTML = '';
  addRow(container, catalog);
  const addBtn = document.createElement('button');
  addBtn.className = 'btn-secondary';
  addBtn.textContent = '+ Ингредиент';
  addBtn.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
  addBtn.onclick = () => {
    container.insertBefore(createRow(catalog), addBtn);
  };
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
  row.innerHTML = `
    <div class="ingr-row-main">
      <input class="form-input ingr-name-input" placeholder="Ингредиент..." autocomplete="off">
      <input class="form-input ingr-amount-input" type="number" placeholder="100">
      <div class="ingr-unit-acc">
        <button type="button" class="ingr-unit-btn">${selUnit} ▾</button>
        <div class="ingr-unit-dropdown" style="display:none">
          ${UNITS.map(u => `<div class="ingr-unit-opt${u === selUnit ? ' active' : ''}" data-val="${u}">${u}</div>`).join('')}
        </div>
      </div>
      <button type="button" class="ingr-del-btn">&times;</button>
    </div>`;
  // Unit accordion
  const unitBtn = row.querySelector('.ingr-unit-btn');
  const unitDD = row.querySelector('.ingr-unit-dropdown');
  unitBtn.onclick = (e) => {
    e.preventDefault();
    unitDD.style.display = unitDD.style.display === 'none' ? '' : 'none';
  };
  unitDD.querySelectorAll('.ingr-unit-opt').forEach(opt => {
    opt.onclick = () => {
      unitDD.querySelectorAll('.ingr-unit-opt').forEach(o => o.classList.remove('active'));
      opt.classList.add('active');
      unitBtn.textContent = `${opt.dataset.val} ▾`;
      unitDD.style.display = 'none';
    };
  });
  // Close dropdown on outside click
  row.addEventListener('focusout', () => setTimeout(() => { unitDD.style.display = 'none'; }, 150));
  // Delete row (keep at least 1)
  row.querySelector('.ingr-del-btn').onclick = () => {
    if (row.parentElement.querySelectorAll('.ingr-row').length > 1) row.remove();
  };
  // Autocomplete
  const nameInput = row.querySelector('.ingr-name-input');
  nameInput.addEventListener('input', () => showAutocomplete(row, nameInput, catalog));
  nameInput.addEventListener('blur', () => setTimeout(() => closeAC(row), 150));
  return row;
}

function showAutocomplete(row, input, catalog) {
  closeAC(row);
  const q = input.value.trim().toLowerCase();
  if (!q) return;
  const matches = catalog.filter(c => c.name.toLowerCase().includes(q)).slice(0, 8);
  const dd = document.createElement('div');
  dd.className = 'ingr-autocomplete';
  for (const item of matches) {
    const opt = document.createElement('div');
    opt.className = 'ingr-autocomplete-item';
    opt.textContent = item.name;
    opt.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row); };
    dd.appendChild(opt);
  }
  if (!matches.some(m => m.name.toLowerCase() === q)) {
    const create = document.createElement('div');
    create.className = 'ingr-autocomplete-item ingr-autocomplete-create';
    create.textContent = `+ Создать «${input.value.trim()}»`;
    create.onmousedown = async (e) => {
      e.preventDefault();
      const name = input.value.trim();
      const cat = ingrCat(name) || 'other';
      try { await invoke('add_ingredient_to_catalog', { name, category: cat }); catalog.push({ name, category: cat }); } catch {}
      selectItem(input, name, row);
    };
    dd.appendChild(create);
  }
  if (dd.children.length) row.querySelector('.ingr-row-main').appendChild(dd);
}

function selectItem(input, name, row) {
  input.value = name;
  closeAC(row);
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
