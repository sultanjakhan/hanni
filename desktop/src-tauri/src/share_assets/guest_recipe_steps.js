// guest_recipe_steps.js — Structured cooking steps (copy of food-recipe-steps.js).
(function () {
  const u = (window.HanniGuest || {}).utils;
  if (!u) return;

  const ACTIONS = [
    'Жарить', 'Варить', 'Тушить', 'Запекать', 'Нарезать', 'Смешать',
    'Обжарить', 'Залить', 'Добавить', 'Довести до кипения', 'Остудить',
    'Натереть', 'Замариновать', 'Отварить', 'Подать',
  ];

  function renderStepsRows(container, ingredientsFn) {
    container.innerHTML = '';
    container.appendChild(createStepRow(ingredientsFn));
    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.className = 'btn-secondary';
    addBtn.textContent = '+ Шаг';
    addBtn.style.cssText = 'margin-top:6px;font-size:12px;padding:4px 10px;';
    addBtn.onclick = () => {
      container.insertBefore(createStepRow(ingredientsFn), addBtn);
      updateNumbers(container);
    };
    container.appendChild(addBtn);
  }

  function createStepRow(ingredientsFn) {
    const row = document.createElement('div');
    row.className = 'step-row';
    row.innerHTML = `
      <span class="step-num">1.</span>
      <div class="step-fields">
        <div class="step-line">
          <div class="step-acc-wrap">
            <button type="button" class="step-acc-btn step-prod-btn">Продукт ▾</button>
            <div class="step-dropdown" data-for="prod" style="display:none"></div>
          </div>
          <div class="step-acc-wrap">
            <button type="button" class="step-acc-btn step-action-btn">Действие ▾</button>
            <div class="step-dropdown" data-for="action" style="display:none">
              ${ACTIONS.map(a => `<div class="step-dd-opt" data-val="${a}">${a}</div>`).join('')}
            </div>
          </div>
          <input class="form-input step-time-input" type="number" placeholder="мин">
        </div>
        <input class="form-input step-note-input" placeholder="Доп. (вращать каждые N мин, помешивать...)">
      </div>
      <button type="button" class="ingr-del-btn step-del">&times;</button>`;
    setupDropdown(row, '.step-action-btn', '[data-for="action"]', (val) => {
      row.querySelector('.step-action-btn').textContent = `${val} ▾`;
    });
    const prodBtn = row.querySelector('.step-prod-btn');
    const prodDD = row.querySelector('[data-for="prod"]');
    prodBtn.onclick = (e) => {
      e.preventDefault();
      const open = prodDD.style.display !== 'none';
      if (open) { prodDD.style.display = 'none'; return; }
      const names = ingredientsFn();
      prodDD.innerHTML = names.length
        ? names.map(n => `<div class="step-dd-opt" data-val="${n}">${n}</div>`).join('')
        : '<div class="step-dd-opt" style="color:var(--text-muted)">Добавьте ингредиенты</div>';
      prodDD.style.display = '';
      prodDD.querySelectorAll('.step-dd-opt[data-val]').forEach(opt => {
        opt.onclick = () => { prodBtn.textContent = `${opt.dataset.val} ▾`; prodDD.style.display = 'none'; };
      });
    };
    row.addEventListener('focusout', () => setTimeout(() => { prodDD.style.display = 'none'; }, 150));
    row.querySelector('.step-del').onclick = () => {
      if (row.parentElement.querySelectorAll('.step-row').length > 1) {
        row.remove();
        updateNumbers(row.parentElement || document);
      }
    };
    return row;
  }

  function setupDropdown(row, btnSel, ddSel, onSelect) {
    const btn = row.querySelector(btnSel);
    const dd = row.querySelector(ddSel);
    btn.onclick = (e) => { e.preventDefault(); dd.style.display = dd.style.display === 'none' ? '' : 'none'; };
    dd.querySelectorAll('.step-dd-opt').forEach(opt => {
      opt.onclick = () => { onSelect(opt.dataset.val); dd.style.display = 'none'; };
    });
    row.addEventListener('focusout', () => setTimeout(() => { dd.style.display = 'none'; }, 150));
  }

  function updateNumbers(container) {
    container.querySelectorAll('.step-row').forEach((row, i) => {
      const num = row.querySelector('.step-num');
      if (num) num.textContent = `${i + 1}.`;
    });
  }

  function collectSteps(container) {
    const steps = [];
    container.querySelectorAll('.step-row').forEach(row => {
      const prod = row.querySelector('.step-prod-btn')?.textContent?.replace(' ▾', '').trim();
      const action = row.querySelector('.step-action-btn')?.textContent?.replace(' ▾', '').trim();
      const time = row.querySelector('.step-time-input')?.value?.trim();
      const note = row.querySelector('.step-note-input')?.value?.trim();
      const parts = [];
      if (prod && prod !== 'Продукт') parts.push(prod);
      if (action && action !== 'Действие') parts.push(action.toLowerCase());
      if (time) parts.push(`${time} мин`);
      if (note) parts.push(note);
      if (parts.length) steps.push(parts.join(', '));
    });
    return steps.join('\n');
  }

  window.HanniGuest = window.HanniGuest || {};
  window.HanniGuest.recipeSteps = { renderStepsRows, collectSteps };
})();
