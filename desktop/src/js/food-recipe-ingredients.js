// ── food-recipe-ingredients.js — Structured ingredient input: category → product → create ──
import { invoke } from './state.js';
import { CAT_LABELS, CAT_ORDER, invalidateCatalogCache } from './food-recipe-filters.js';

const UNITS = ['г', 'кг', 'мл', 'л', 'шт', 'ст.л.', 'ч.л.', 'стакан'];

export function renderIngredientRows(ct, catalog) {
  ct.innerHTML = '';
  ct.insertBefore(createRow(catalog), null);
  const ab = document.createElement('button');
  ab.className = 'btn-secondary'; ab.textContent = '+ Ингредиент';
  ab.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
  ab.onclick = () => { ct.insertBefore(createRow(catalog), ab); };
  ct.appendChild(ab);
}

function createRow(catalog) {
  const row = document.createElement('div');
  row.className = 'ingr-row';
  const selUnit = UNITS[0];
  row.innerHTML = `<div class="ingr-row-main">
    <input class="form-input ingr-name-input" placeholder="Категория..." autocomplete="off" readonly>
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
  nameInput.addEventListener('focus', () => openPicker(row, nameInput, catalog));
  nameInput.addEventListener('blur', () => setTimeout(() => closeAC(row), 180));
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
function openPicker(row, input, catalog) {
  if (row.dataset.cat) showProductList(row, input, catalog, row.dataset.cat);
  else showCategoryPicker(row, input, catalog);
}
function showCategoryPicker(row, input, catalog) {
  closeAC(row);
  const dd = document.createElement('div');
  dd.className = 'ingr-autocomplete ingr-cat-picker';
  for (const cat of CAT_ORDER) {
    const count = catalog.filter(c => c.category === cat).length;
    const opt = document.createElement('div');
    opt.className = 'ingr-autocomplete-item ingr-cat-btn';
    opt.innerHTML = `${CAT_LABELS[cat] || cat} <span class="ingr-cat-count">(${count})</span>`;
    opt.onmousedown = (e) => {
      e.preventDefault();
      row.dataset.cat = cat;
      input.readOnly = false;
      input.placeholder = `Поиск в ${CAT_LABELS[cat]}...`;
      input.value = '';
      showProductList(row, input, catalog, cat);
      input.focus();
    };
    dd.appendChild(opt);
  }
  row.querySelector('.ingr-row-main').appendChild(dd);
}

function showProductList(row, input, catalog, cat) {
  closeAC(row);
  const q = input.value.trim().toLowerCase();
  const items = catalog.filter(c => c.category === cat && (!q || c.name.toLowerCase().includes(q)));
  const dd = document.createElement('div');
  dd.className = 'ingr-autocomplete';
  // Back link
  const back = document.createElement('div');
  back.className = 'ingr-autocomplete-item ingr-back-link';
  back.textContent = `← ${CAT_LABELS[cat]}`;
  back.onmousedown = (e) => {
    e.preventDefault();
    delete row.dataset.cat;
    input.readOnly = true;
    input.placeholder = 'Категория...';
    input.value = '';
    showCategoryPicker(row, input, catalog);
  };
  dd.appendChild(back);
  // Products
  for (const item of items.slice(0, 15)) {
    const opt = document.createElement('div');
    opt.className = 'ingr-autocomplete-item ingr-product-row';
    const nameSpan = document.createElement('span');
    nameSpan.textContent = item.name;
    nameSpan.onmousedown = (e) => { e.preventDefault(); selectItem(input, item.name, row); };
    const infoBtn = document.createElement('span');
    infoBtn.className = 'ingr-info-btn';
    infoBtn.textContent = 'ⓘ';
    infoBtn.onmousedown = async (e) => {
      e.preventDefault(); e.stopPropagation(); closeAC(row);
      const { showProductPopup } = await import('./food-product-popup.js');
      showProductPopup(input, item, catalog, () => showProductList(row, input, catalog, cat));
    };
    opt.append(nameSpan, infoBtn);
    dd.appendChild(opt);
  }
  // Create option
  if (q && !items.some(i => i.name.toLowerCase() === q)) {
    const create = document.createElement('div');
    create.className = 'ingr-autocomplete-item ingr-autocomplete-create';
    create.textContent = `+ Создать «${input.value.trim()}»`;
    create.onmousedown = async (e) => {
      e.preventDefault();
      const nm = input.value.trim();
      try { await invoke('add_ingredient_to_catalog', { name: nm, category: cat }); catalog.push({ name: nm, category: cat }); invalidateCatalogCache(); } catch {}
      selectItem(input, nm, row);
    };
    dd.appendChild(create);
  }
  row.querySelector('.ingr-row-main').appendChild(dd);
  // Live filter on input
  input.oninput = () => showProductList(row, input, catalog, cat);
}

function selectItem(input, name, row) {
  input.value = name;
  input.readOnly = true;
  input.oninput = null;
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
