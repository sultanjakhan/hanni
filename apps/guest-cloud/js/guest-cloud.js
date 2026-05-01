// guest-cloud.js — read-only Firestore-backed guest UI (Stage A MVP).
//
// Mirrors the layout of the axum-served guest, but reads recipes / catalog /
// products / blacklist directly from Firestore. Writes (add/edit/delete) are
// deferred to Stage B which needs a Cloud Function to mint a custom JWT
// containing `share_token` (Firestore rules check that claim).
//
// For now the UI gracefully hides write actions until the JWT bridge is in.

export async function boot() {
  const C = window.__HANNI_CLOUD__;
  const { db, doc, getDoc, getDocs, collection, query, where, orderBy } = C;
  const app = document.getElementById('app');

  // 1. Load share-link doc to discover scope/permissions/owner.
  const linkSnap = await getDoc(doc(db, 'share_links', C.shareToken));
  if (!linkSnap.exists()) {
    app.innerHTML = `<div class="err">Ссылка не найдена или отозвана.</div>`;
    return;
  }
  const link = linkSnap.data();
  if (link.revoked_at) {
    app.innerHTML = `<div class="err">Эта ссылка отозвана.</div>`;
    return;
  }
  if (link.expires_at && link.expires_at.toMillis() < Date.now()) {
    app.innerHTML = `<div class="err">Срок действия ссылки истёк.</div>`;
    return;
  }
  document.getElementById('hdr-title').textContent = link.label || 'Hanni';
  const ownerUid = link.owner_uid;
  const scope = link.scope;
  const perms = link.permissions || [];
  const can = (p) => perms.includes(p);
  const inScope = (area) => scope === 'all' || scope === area;

  // 2. Tab bar
  const tabs = [];
  if (inScope('recipes'))   tabs.push(['recipes',   '🍔 Рецепты']);
  if (inScope('products'))  tabs.push(['products',  '🛒 Продукты']);
  if (inScope('products'))  tabs.push(['fridge',    '🥶 Холодильник']);
  if (inScope('meal_plan')) tabs.push(['meal_plan', '🍽 План']);
  if (scope === 'all')      tabs.push(['memory',    '🧠 Память']);

  let active = tabs[0]?.[0] || 'recipes';

  function renderShell() {
    app.innerHTML = `
      <div class="tab-bar">
        ${tabs.map(([k, l]) => `<button class="tab-btn ${active === k ? 'active' : ''}" data-tab="${k}">${l}</button>`).join('')}
      </div>
      <div id="pane"></div>
    `;
    app.querySelectorAll('[data-tab]').forEach(b => {
      b.onclick = () => { active = b.dataset.tab; renderShell(); renderPane(); };
    });
    renderPane();
  }

  // 3. Pane router
  async function renderPane() {
    const pane = document.getElementById('pane');
    pane.innerHTML = '<div class="muted">Загрузка…</div>';
    try {
      switch (active) {
        case 'recipes':   await renderRecipes(pane);  break;
        case 'products':  await renderCatalog(pane);  break;
        case 'fridge':    await renderFridge(pane);   break;
        case 'meal_plan': await renderMealPlan(pane); break;
        case 'memory':    await renderMemory(pane);   break;
      }
    } catch (e) {
      pane.innerHTML = `<div class="err">Ошибка: ${e.message || e}</div>`;
    }
  }

  // ── Recipes ──────────────────────────────────────────────────────────
  async function renderRecipes(pane) {
    const q = query(collection(db, 'recipes'), where('owner_uid', '==', ownerUid), orderBy('updated_at', 'desc'));
    const snap = await getDocs(q);
    const items = snap.docs.map(d => ({ id: d.id, ...d.data() }));
    if (!items.length) { pane.innerHTML = '<div class="empty">Рецептов пока нет.</div>'; return; }
    pane.innerHTML = `
      <h2>Рецепты (${items.length})</h2>
      <div class="recipe-grid">
        ${items.map(r => `
          <div class="recipe-card" data-id="${r.id}">
            <div class="recipe-card-name">${esc(r.name)}</div>
            <div class="recipe-card-meta muted">${r.servings || 1} порц · ${r.calories || 0} kcal</div>
            <div class="recipe-card-tags">${(r.ingredients || '').split(',').slice(0, 5).map(t => `<span class="ingr-tag">${esc(t.trim())}</span>`).join('')}</div>
          </div>`).join('')}
      </div>`;
    pane.querySelectorAll('.recipe-card').forEach(el => {
      el.onclick = () => openRecipe(el.dataset.id);
    });
  }

  async function openRecipe(id) {
    const pane = document.getElementById('pane');
    pane.innerHTML = '<div class="muted">Загрузка…</div>';
    const [rSnap, ingSnap, comSnap] = await Promise.all([
      getDoc(doc(db, 'recipes', id)),
      getDocs(query(collection(db, 'recipe_ingredients'), where('recipe_id', '==', id))),
      getDocs(query(collection(db, 'recipes', id, 'comments'), orderBy('created_at', 'asc'))),
    ]);
    const r = rSnap.data();
    const ings = ingSnap.docs.map(d => d.data());
    const coms = comSnap.docs.map(d => d.data());
    pane.innerHTML = `
      <button id="back" class="detail-back">← Назад к списку</button>
      <h2>${esc(r.name)}</h2>
      ${r.description ? `<p class="muted">${esc(r.description)}</p>` : ''}
      <div class="recipe-detail-section"><h4>Ингредиенты</h4>
        ${ings.map(i => `<span class="ingr-tag">${esc(i.name)}${i.amount ? ' — ' + i.amount + (i.unit || '') : ''}</span>`).join(' ')}
      </div>
      <div class="recipe-detail-section"><h4>Приготовление</h4>
        ${r.instructions ? `<div>${esc(r.instructions)}</div>` : '<div class="muted">Нет инструкций.</div>'}
      </div>
      <div class="recipe-detail-section"><h4>Заметки гостей</h4>
        ${coms.length ? coms.map(c => `<div class="comment"><b>${esc(c.author)}</b>: ${esc(c.text)}</div>`).join('') : '<div class="muted">Пока нет.</div>'}
      </div>
    `;
    pane.querySelector('#back').onclick = () => renderPane();
  }

  // ── Catalog (Продукты) ───────────────────────────────────────────────
  async function renderCatalog(pane) {
    const q = collection(db, 'ingredient_catalog');
    const snap = await getDocs(q);
    const items = snap.docs.map(d => d.data()).filter(p => p.owner_uid === ownerUid);
    pane.innerHTML = `<h2>Каталог (${items.length})</h2>
      <div class="grid">${items.slice(0, 100).map(p => `<div class="muted">${esc(p.name)} <small>${esc(p.category || '')}</small></div>`).join('')}</div>`;
  }

  // ── Fridge ───────────────────────────────────────────────────────────
  async function renderFridge(pane) {
    const q = query(collection(db, 'products'), where('owner_uid', '==', ownerUid));
    const snap = await getDocs(q);
    const items = snap.docs.map(d => ({ id: d.id, ...d.data() }));
    pane.innerHTML = `<h2>Холодильник (${items.length})</h2>
      <div class="product-grid">${items.map(p => `
        <div class="product-card">
          <div class="product-card-name">${esc(p.name)}</div>
          <div class="product-card-tags">
            <span class="product-card-tag">${p.quantity ?? 1} ${esc(p.unit || 'шт')}</span>
            <span class="product-card-tag">${esc(p.location || 'fridge')}</span>
          </div>
        </div>`).join('')}</div>`;
  }

  // ── Meal plan ────────────────────────────────────────────────────────
  async function renderMealPlan(pane) {
    const today = new Date().toISOString().slice(0, 10);
    const q = query(collection(db, 'meal_plan'), where('owner_uid', '==', ownerUid), where('date', '==', today));
    const snap = await getDocs(q);
    const items = snap.docs.map(d => d.data());
    pane.innerHTML = `<h2>План на ${today}</h2>
      ${items.length
        ? `<ul>${items.map(i => `<li>${esc(i.meal_type)}: ${esc(i.recipe_name || i.recipe_id)}</li>`).join('')}</ul>`
        : '<div class="muted">На эту дату планов нет.</div>'}`;
  }

  // ── Memory (blacklist) ───────────────────────────────────────────────
  async function renderMemory(pane) {
    const snap = await getDocs(query(collection(db, 'food_blacklist'), where('owner_uid', '==', ownerUid)));
    const items = snap.docs.map(d => d.data());
    const groups = { product: [], tag: [], category: [], keyword: [] };
    for (const e of items) (groups[e.type] ||= []).push(e.value);
    pane.innerHTML = `<h2>Память (${items.length})</h2>
      ${Object.entries(groups).filter(([_, v]) => v.length).map(([t, vs]) =>
        `<div class="muted">${t}: ${vs.map(v => `<span class="ingr-tag">${esc(v)}</span>`).join(' ')}</div>`
      ).join('') || '<div class="muted">Блэклист пуст.</div>'}`;
  }

  // ── helpers ──────────────────────────────────────────────────────────
  function esc(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  }

  renderShell();
}
