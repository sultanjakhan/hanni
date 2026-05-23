// recipe-shared-steps.js — Structured cooking steps for the recipe wizard.
// Loaded as a plain <script> BEFORE recipe-shared.js. Registers helpers under
// window.HanniRecipe.steps. A step = { text, min, ingredients[] }; the per-step
// product chips are picked from the ingredients entered on the wizard's step 2.
(function () {
  // ── Public: render the step builder into `ct`. ──
  // getIngredients() → [{name,...}] of the current recipe ingredients (live).
  function renderStepRows(ct, getIngredients, rawInstructions) {
    ct.innerHTML = '';
    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.className = 'btn-secondary step-add-btn';
    addBtn.textContent = '+ Шаг';
    addBtn.onclick = () => {
      const card = createStepCard(ct, getIngredients);
      ct.insertBefore(card, addBtn);
      renumber(ct);
      card.querySelector('.step-note-input')?.focus();
    };
    const initial = parseSteps(rawInstructions);
    if (initial.length) for (const st of initial) ct.appendChild(createStepCard(ct, getIngredients, st));
    else ct.appendChild(createStepCard(ct, getIngredients));
    ct.appendChild(addBtn);
    renumber(ct);
  }

  // Parse stored instructions: JSON array of {text,min,ingredients} or [] for legacy/empty.
  function parseSteps(raw) {
    const s = (raw || '').trim();
    if (!s.startsWith('[')) return [];
    try {
      const arr = JSON.parse(s);
      if (Array.isArray(arr)) return arr
        .map(x => ({ text: String(x.text || ''), min: x.min || 0, ingredients: Array.isArray(x.ingredients) ? x.ingredients : [] }))
        .filter(x => x.text);
    } catch { /* ignore */ }
    return [];
  }

  function renumber(ct) {
    ct.querySelectorAll('.step-card').forEach((r, i) => {
      const n = r.querySelector('.step-num'); if (n) n.textContent = String(i + 1);
    });
  }

  function move(ct, card, dir) {
    if (dir < 0 && card.previousElementSibling?.classList.contains('step-card'))
      ct.insertBefore(card, card.previousElementSibling);
    else if (dir > 0 && card.nextElementSibling?.classList.contains('step-card'))
      ct.insertBefore(card.nextElementSibling, card);
    renumber(ct);
  }

  function createStepCard(ct, getIngredients, step) {
    const card = document.createElement('div');
    card.className = 'step-card';
    card.innerHTML = `<div class="step-num">1</div>
      <div class="step-card-body">
        <input class="form-input step-note-input" placeholder="Что делаем на этом шаге">
        <div class="step-card-meta">
          <input class="form-input step-time-input" type="number" min="0" placeholder="мин">
          <span style="flex:1"></span>
          <button type="button" class="step-icon-btn step-up" title="Вверх">↑</button>
          <button type="button" class="step-icon-btn step-down" title="Вниз">↓</button>
          <button type="button" class="step-icon-btn step-del" title="Удалить шаг">&times;</button>
        </div>
        <div class="step-prods"></div>
      </div>`;
    const prods = card.querySelector('.step-prods');
    renderProdAdder(prods, getIngredients);
    card.querySelector('.step-up').onclick = () => move(ct, card, -1);
    card.querySelector('.step-down').onclick = () => move(ct, card, 1);
    card.querySelector('.step-del').onclick = () => {
      if (ct.querySelectorAll('.step-card').length > 1) { card.remove(); renumber(ct); }
    };
    if (step) {
      card.querySelector('.step-note-input').value = step.text || '';
      if (step.min) card.querySelector('.step-time-input').value = step.min;
      const add = prods.querySelector('.step-prod-add');
      for (const n of (step.ingredients || [])) addProdChip(prods, add, n);
    }
    return card;
  }

  // Selected products live as chips inside `.step-prods`; a trailing "+" chip
  // opens a dropdown of the recipe's ingredients not yet chosen for this step.
  function renderProdAdder(prods, getIngredients) {
    const add = document.createElement('button');
    add.type = 'button';
    add.className = 'rf-chip rf-chip-add step-prod-add';
    add.textContent = '+ продукт';
    add.onclick = (e) => { e.preventDefault(); openProdPicker(prods, add, getIngredients); };
    prods.appendChild(add);
  }

  function chosenNames(prods) {
    return [...prods.querySelectorAll('.step-prod-chip')].map(c => c.dataset.name.toLowerCase());
  }

  function openProdPicker(prods, anchor, getIngredients) {
    prods.querySelector('.ingr-autocomplete')?.remove();
    const taken = new Set(chosenNames(prods));
    const names = [...new Set((getIngredients() || []).map(i => i.name).filter(Boolean))]
      .filter(n => !taken.has(n.toLowerCase()));
    const dd = document.createElement('div');
    dd.className = 'ingr-autocomplete';
    dd.style.cssText = 'position:absolute;z-index:1200;min-width:160px;';
    if (!names.length) {
      const empty = document.createElement('div');
      empty.className = 'ingr-autocomplete-item';
      empty.style.color = 'var(--text-muted)';
      empty.textContent = 'Сначала добавь продукты (шаг 2)';
      dd.appendChild(empty);
    } else {
      for (const n of names) {
        const opt = document.createElement('div');
        opt.className = 'ingr-autocomplete-item';
        opt.textContent = n;
        opt.onmousedown = (e) => { e.preventDefault(); addProdChip(prods, anchor, n); dd.remove(); };
        dd.appendChild(opt);
      }
    }
    prods.appendChild(dd);
    const off = (e) => { if (!dd.contains(e.target) && e.target !== anchor) { dd.remove(); document.removeEventListener('mousedown', off); } };
    setTimeout(() => document.addEventListener('mousedown', off), 10);
  }

  function addProdChip(prods, anchor, name) {
    const chip = document.createElement('span');
    chip.className = 'step-prod-chip';
    chip.dataset.name = name;
    chip.textContent = name;
    const x = document.createElement('button');
    x.type = 'button';
    x.className = 'step-prod-x';
    x.textContent = '×';
    x.onclick = () => chip.remove();
    chip.appendChild(x);
    prods.insertBefore(chip, anchor);
  }

  function collectSteps(ct) {
    const steps = [];
    ct.querySelectorAll('.step-card').forEach(card => {
      const text = card.querySelector('.step-note-input')?.value?.trim();
      if (!text) return;
      const min = parseInt(card.querySelector('.step-time-input')?.value) || 0;
      const ingredients = [...card.querySelectorAll('.step-prod-chip')].map(c => c.dataset.name);
      steps.push({ text, min, ingredients });
    });
    return steps;
  }

  window.HanniRecipe = window.HanniRecipe || {};
  window.HanniRecipe.steps = { renderStepRows, collectSteps };
})();
