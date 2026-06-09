// ── food-cooking-log.js — Log a cooking of a recipe (date + rating + note) ──
// Opened from the calendar "+" menu. Creates a "Готовка" calendar event and an
// immutable cooking_log entry with a per-day taste rating + note.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

// Convert `amount` between units of the same family (mass: г/кг, volume: мл/л).
// Returns null when units are incompatible (e.g. г vs шт) so the caller can fall
// back to deducting a single unit.
function convertAmount(amount, fromUnit, toUnit) {
  const u = (x) => String(x || '').trim().toLowerCase().replace(/\.$/, '');
  const f = u(fromUnit), t = u(toUnit);
  if (f === t) return amount;
  const MASS = { 'г': 1, 'гр': 1, 'кг': 1000 };
  const VOL = { 'мл': 1, 'л': 1000 };
  for (const fam of [MASS, VOL]) {
    if (f in fam && t in fam) return amount * fam[f] / fam[t];
  }
  return null;
}

export function showCookingLogModal(date, onSaved, preRecipe, preEventId) {
  const today = date || new Date().toISOString().slice(0, 10);
  let selectedId = preRecipe ? preRecipe.id : null;
  let selectedName = preRecipe ? preRecipe.name : '';
  let rating = 0, allRecipes = [], deductMatches = [];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">🍳 Отметить готовку</div>
    <div class="form-group"><label class="form-label">Дата</label>
      <input class="form-input" id="cl-date" type="date" value="${today}"></div>
    <div class="form-group"><label class="form-label">Рецепт</label>
      <input class="form-input" id="cl-search" placeholder="Поиск рецепта...">
      <div id="cl-list" class="mp-recipe-list" style="max-height:180px;overflow-y:auto;margin-top:6px;"></div></div>
    <div id="cl-deduct"></div>
    <div class="form-group"><label class="form-label">Оценка вкуса</label>
      <div class="rd-stars" id="cl-stars">${[1, 2, 3, 4, 5].map(n => `<span class="rd-star" data-n="${n}">★</span>`).join('')}</div></div>
    <div class="form-group"><label class="form-label">Заметка</label>
      <textarea class="form-textarea" id="cl-note" rows="2" placeholder="Как вышло, что поменять в следующий раз…"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="cl-cancel">Отмена</button>
      <button class="btn-primary" id="cl-save" disabled>Сохранить</button>
    </div></div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('#cl-cancel').onclick = close;

  const stars = overlay.querySelectorAll('.rd-star');
  const paintStars = (n) => stars.forEach(x => x.classList.toggle('on', parseInt(x.dataset.n) <= n));
  stars.forEach(s => {
    s.onclick = () => { rating = parseInt(s.dataset.n); paintStars(rating); };
    s.onmouseenter = () => paintStars(parseInt(s.dataset.n)); // hover preview
  });
  overlay.querySelector('#cl-stars').onmouseleave = () => paintStars(rating); // restore actual

  const search = overlay.querySelector('#cl-search');
  search.oninput = () => renderList(search.value.trim().toLowerCase());
  (async () => {
    allRecipes = await invoke('get_recipes', { search: null }).catch(() => []);
    renderList('');
    if (selectedId) { overlay.querySelector('#cl-save').disabled = false; loadDeduct(); } // preselected recipe → ready
  })();

  function renderList(q) {
    const list = overlay.querySelector('#cl-list');
    const filtered = q ? allRecipes.filter(r => r.name.toLowerCase().includes(q)) : allRecipes;
    list.innerHTML = filtered.map(r =>
      `<div class="mp-recipe-option${r.id === selectedId ? ' selected' : ''}" data-rid="${r.id}"><span>${escapeHtml(r.name)}</span></div>`
    ).join('') || '<div style="padding:8px;color:var(--text-muted);font-size:13px;">Нет рецептов</div>';
    list.querySelectorAll('.mp-recipe-option').forEach(opt => opt.onclick = () => {
      selectedId = parseInt(opt.dataset.rid);
      selectedName = allRecipes.find(r => r.id === selectedId)?.name || '';
      list.querySelectorAll('.mp-recipe-option').forEach(o => o.classList.toggle('selected', o === opt));
      overlay.querySelector('#cl-save').disabled = false;
      loadDeduct();
    });
  }

  // Match the selected recipe's ingredients to fridge products so we can offer
  // to deduct them on save (units rarely line up, so it's an opt-in checklist).
  const clNorm = (s) => String(s == null ? '' : s).trim().toLowerCase();
  async function loadDeduct() {
    const box = overlay.querySelector('#cl-deduct');
    if (!selectedId) { box.innerHTML = ''; deductMatches = []; return; }
    box.innerHTML = '<div style="color:var(--text-muted);font-size:12px">Проверяю холодильник…</div>';
    const [rec, products] = await Promise.all([
      invoke('get_recipe', { id: selectedId }).catch(() => null),
      invoke('get_products', {}).catch(() => []),
    ]);
    const ings = (rec && rec.ingredient_items) || [];
    deductMatches = [];
    for (const ing of ings) {
      const p = products.find(x =>
        (ing.catalog_id && x.catalog_id && x.catalog_id === ing.catalog_id) || clNorm(x.name) === clNorm(ing.name));
      if (p) deductMatches.push({ ing, p });
    }
    if (!deductMatches.length) { box.innerHTML = ''; return; }
    box.innerHTML = `<div class="form-group"><label class="form-label">Списать из холодильника</label>
      ${deductMatches.map((m, i) => {
        const conv = m.ing.amount ? convertAmount(m.ing.amount, m.ing.unit, m.p.unit) : null;
        const dec = (conv != null) ? Math.round(conv * 100) / 100 : 1;
        const un = escapeHtml(m.p.unit || 'шт');
        return `<label style="display:flex;gap:8px;align-items:center;font-size:13px;padding:2px 0">
        <input type="checkbox" class="cl-deduct-cb" data-i="${i}" checked>
        ${escapeHtml(m.p.name)} <span style="color:var(--text-muted)">(есть ${m.p.quantity ?? 1} ${un} → спишется ${dec} ${un})</span>
      </label>`;
      }).join('')}</div>`;
  }

  overlay.querySelector('#cl-save').onclick = async () => {
    if (!selectedId) return;
    const d = overlay.querySelector('#cl-date').value || today;
    const note = overlay.querySelector('#cl-note').value.trim();
    try {
      // Reuse the event created when cooking started; create one only for the
      // log-only path (manual "уже приготовил", no cook session).
      let eventId = preEventId ?? null;
      if (eventId == null) {
        let color = '#cb8a05';
        const cats = await invoke('list_event_categories').catch(() => []);
        const cat = cats.find(c => c.name === 'Готовка');
        if (cat) color = cat.color;
        eventId = await invoke('create_event', {
          title: selectedName, description: '', date: d, time: '',
          durationMinutes: 30, category: 'Готовка', color, priority: null,
        }).catch(() => null);
      }
      await invoke('log_cooking', { recipeId: selectedId, date: d, tasteRating: rating, cookNote: note, eventId });
      // Deduct the checked fridge items: by amount when units match, else one unit.
      for (const cb of overlay.querySelectorAll('.cl-deduct-cb:checked')) {
        const m = deductMatches[parseInt(cb.dataset.i)]; if (!m) continue;
        const conv = m.ing.amount ? convertAmount(m.ing.amount, m.ing.unit, m.p.unit) : null;
        const dec = (conv != null) ? conv : 1; // convert across compatible units, else one unit
        const next = Math.round(((parseFloat(m.p.quantity) || 0) - dec) * 100) / 100;
        try {
          if (next <= 0) await invoke('delete_product', { id: m.p.id });
          else {
            const args = { id: m.p.id, name: m.p.name, quantity: next, expiryDate: m.p.expiry_date, location: m.p.location, notes: m.p.notes };
            if (m.p.catalog_id != null) args.catalogId = m.p.catalog_id;
            await invoke('update_product', args);
          }
        } catch (_) { /* best-effort deduction */ }
      }
      close();
      if (onSaved) await onSaved();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  };
}

// ── "Что приготовить" — rank recipes by what's in the fridge, filter by
// ingredient name or category. Opened from the calendar templates; picking a
// recipe launches the guided cook mode, which logs the cook on finish. ──
const CW_CAT_LABELS = { meat: 'Мясо', fish: 'Рыба', veg: 'Овощи', fruit: 'Фрукты',
  grain: 'Крупы', dairy: 'Молочные', legumes: 'Бобовые', nuts: 'Орехи', spice: 'Специи',
  oil: 'Масла', bakery: 'Выпечка', drinks: 'Напитки', sweet: 'Сладости', frozen: 'Заморозка', other: 'Другое' };
const cwNorm = (s) => String(s == null ? '' : s).trim().toLowerCase();
const cwIngredients = (str) => String(str || '').split(/[,;\n]/).map(s => s.trim()).filter(Boolean);
const cwName = (n) => n.split(':')[0].trim(); // "соль: 2ч.л." → "соль" (match fridge by name)

export async function showCookWhatModal(date, onSaved, searchSeed) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:520px;max-height:90vh;overflow-y:auto">
    <div class="modal-title">🍲 Что приготовить</div>
    <input class="form-input" id="cw-search" placeholder="Поиск по рецепту или ингредиенту…" style="margin-bottom:8px">
    <div id="cw-cats" style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px"></div>
    <div id="cw-list"><div class="muted">Загрузка…</div></div>
    <div class="modal-actions" style="justify-content:space-between">
      <button id="cw-justlog" style="background:none;border:none;color:var(--text-secondary);cursor:pointer;font-size:13px;text-decoration:underline;padding:0">Уже приготовил — отметить</button>
      <button class="btn-secondary" id="cw-close">Закрыть</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('#cw-close').onclick = close;
  // Skip the cook flow → log a cook that happened off-app (no timer, pick recipe in the log).
  overlay.querySelector('#cw-justlog').onclick = () => { close(); showCookingLogModal(date, onSaved); };

  const [recipes, products, catalog] = await Promise.all([
    invoke('get_recipes', { search: null }).catch(() => []),
    invoke('get_products', {}).catch(() => []),
    invoke('get_ingredient_catalog').catch(() => []),
  ]);
  const catByName = new Map(catalog.map(c => [cwNorm(c.name), c.category]));
  const have = new Set(products.map(p => cwNorm(p.name)));

  // Per-recipe: which of its ingredients are missing from the fridge + the
  // catalog categories its ingredients span (for the category filter).
  const rows = recipes.map(r => {
    const ings = cwIngredients(r.ingredients);
    const missing = ings.filter(n => !have.has(cwNorm(cwName(n))));
    const cats = new Set(ings.map(n => catByName.get(cwNorm(cwName(n)))).filter(Boolean));
    return { r, ings, missing, total: ings.length, cats };
  });
  const usedCats = [...new Set(rows.flatMap(x => [...x.cats]))];
  let q = searchSeed || '', cat = 'all';

  function renderCats() {
    overlay.querySelector('#cw-cats').innerHTML = ['all', ...usedCats].map(c =>
      `<button type="button" class="rf-chip ${cat === c ? 'active' : ''}" data-cw-cat="${c}">${c === 'all' ? 'Все' : escapeHtml(CW_CAT_LABELS[c] || c)}</button>`).join('');
    overlay.querySelectorAll('[data-cw-cat]').forEach(b => b.onclick = () => { cat = b.dataset.cwCat; renderCats(); renderList(); });
  }
  function renderList() {
    const ql = cwNorm(q);
    const filtered = rows.filter(x => {
      if (cat !== 'all' && !x.cats.has(cat)) return false;
      if (!ql) return true;
      return cwNorm(x.r.name).includes(ql) || x.ings.some(n => cwNorm(n).includes(ql));
    }).sort((a, b) => (a.missing.length - b.missing.length) || ((b.r.cook_count || 0) - (a.r.cook_count || 0)));
    const list = overlay.querySelector('#cw-list');
    if (!filtered.length) { list.innerHTML = '<div class="empty">Ничего не нашлось.</div>'; return; }
    list.innerHTML = filtered.map(x => {
      const badge = x.missing.length === 0
        ? '<span class="badge badge-green">✓ есть всё</span>'
        : `<span class="badge badge-gray">не хватает ${x.missing.length}: ${escapeHtml(x.missing.slice(0, 3).map(cwName).join(', '))}${x.missing.length > 3 ? '…' : ''}</span>`;
      const addBtn = x.missing.length
        ? `<button class="cw-add-missing" title="Добавить недостающее в список покупок" style="font-size:11px;padding:3px 8px;background:none;border:1px solid var(--border-subtle);border-radius:6px;cursor:pointer;white-space:nowrap">+ в покупки</button>`
        : '';
      return `<div class="cw-row" data-rid="${x.r.id}" data-rname="${escapeHtml(x.r.name)}" style="padding:10px;border:1px solid var(--border-subtle);border-radius:8px;margin-bottom:6px;cursor:pointer">
        <div style="font-weight:600">${escapeHtml(x.r.name)}</div>
        <div style="margin-top:4px;font-size:12px;display:flex;justify-content:space-between;align-items:center;gap:8px">
          <span>${badge} <span class="muted">· ${x.total - x.missing.length}/${x.total} ингр.</span></span>
          ${addBtn}
        </div>
      </div>`;
    }).join('');
    list.querySelectorAll('.cw-add-missing').forEach(btn => btn.onclick = async (e) => {
      e.stopPropagation();
      const row = rows.find(x => x.r.id === parseInt(btn.closest('.cw-row').dataset.rid));
      if (!row) return;
      const { addShoppingItem } = await import('./shopping-list.js');
      for (const n of row.missing) { const c = n.split(':'); await addShoppingItem(c[0].trim(), (c[1] || '').trim(), '').catch(() => {}); }
      btn.textContent = '✓ в покупках'; btn.disabled = true;
    });
    list.querySelectorAll('.cw-row').forEach(row => row.onclick = async () => {
      close();
      // Picking a recipe starts the guided cook mode (timer); it opens the
      // cooking-log on finish, so the pick → cook → log pipeline stays intact.
      const recipe = await invoke('get_recipe', { id: parseInt(row.dataset.rid) }).catch(() => null);
      if (!recipe) return;
      const { startCookMode } = await import('./food-cook-mode.js');
      startCookMode(recipe, { onSaved, date });
    });
  }
  overlay.querySelector('#cw-search').value = q;
  renderCats(); renderList();
  overlay.querySelector('#cw-search').oninput = (e) => { q = e.target.value; renderList(); };
}
