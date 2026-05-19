// ── food-blacklist-modal.js — Manage the two-level food blacklist ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateBlacklistCache } from './food-recipe-filters.js';
import { applyBlacklist, BL_LEVELS } from './food-blacklist-menu.js';

const TYPE_LABELS = { product: 'Продукт', category: 'Категория', tag: 'Тег', keyword: 'Ключевое слово' };
const BASIC_TYPES = ['product', 'category'];
const ADV_TYPES = ['tag', 'keyword'];

export async function showBlacklistModal(onChange) {
  let entries = await invoke('list_food_blacklist').catch(() => []);
  const catalog = await loadCatalog();
  let addType = 'product';
  let addLevel = 'hard';

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:540px">
    <div class="modal-title">🚫 Блэклист еды</div>
    <p style="color:var(--text-secondary);font-size:13px;margin:0 0 14px;">
      «Не ем» — скрывается из рецептов и каталога. «Не люблю» — остаётся, но тусклее и ниже в списках.</p>
    <div class="bl-sections"></div>
    <div class="bl-add">
      <div class="bl-add-picks">
        <div class="bl-pick bl-pick-level"></div>
        <div class="bl-pick bl-pick-type"></div>
      </div>
      <details class="bl-adv"><summary>⚙ Продвинутое (тег, ключевое слово)</summary>
        <div class="bl-pick bl-pick-type-adv" style="margin-top:8px"></div>
      </details>
      <div class="bl-add-row">
        <input class="form-input bl-value" placeholder="Значение…" autocomplete="off" list="bl-prod-list">
        <datalist id="bl-prod-list">${catalog.map(c => `<option value="${escapeHtml(c.name)}">`).join('')}</datalist>
        <datalist id="bl-cat-list">${CAT_ORDER.map(c => `<option value="${escapeHtml(CAT_LABELS[c])}">`).join('')}</datalist>
        <button class="btn-primary bl-add-btn">Добавить</button>
      </div>
      <div class="bl-hint" style="color:var(--text-muted);font-size:12px;margin-top:6px"></div>
    </div>
    <div class="modal-actions"><button class="btn-secondary" data-close>Закрыть</button></div>
  </div>`;

  const q = (s) => overlay.querySelector(s);
  const valueInp = q('.bl-value');
  const hintEl = q('.bl-hint');

  const chip = (group, val, label, active) =>
    `<button class="bl-tab${active ? ' active' : ''}" data-${group}="${val}">${label}</button>`;

  function renderPicks() {
    q('.bl-pick-level').innerHTML = BL_LEVELS.map(l =>
      chip('lvl', l.level, `${l.icon} ${l.label}`, l.level === addLevel)).join('');
    q('.bl-pick-type').innerHTML = BASIC_TYPES.map(t =>
      chip('type', t, TYPE_LABELS[t], t === addType)).join('');
    q('.bl-pick-type-adv').innerHTML = ADV_TYPES.map(t =>
      chip('type', t, TYPE_LABELS[t], t === addType)).join('');
    overlay.querySelectorAll('[data-lvl]').forEach(b => b.onclick = () => { addLevel = b.dataset.lvl; renderPicks(); });
    overlay.querySelectorAll('[data-type]').forEach(b => b.onclick = () => { addType = b.dataset.type; renderPicks(); renderHint(); });
  }

  function renderSections() {
    q('.bl-sections').innerHTML = BL_LEVELS.map(l => {
      const items = entries.filter(e => (e.level || 'hard') === l.level);
      const chips = items.length ? items.map(e => {
        const disp = e.type === 'category' ? (CAT_LABELS[e.value] || e.value) : e.value;
        return `<span class="bl-chip bl-chip--${l.level}">${escapeHtml(disp)}
          <small>${TYPE_LABELS[e.type] || e.type}</small>
          <button data-del="${e.id}" title="Убрать">×</button></span>`;
      }).join('') : '<span style="color:var(--text-muted);font-size:12px">пусто</span>';
      return `<div class="bl-section">
        <div class="bl-section-head">${l.icon} ${l.label}</div>
        <div class="bl-chips">${chips}</div></div>`;
    }).join('');
    overlay.querySelectorAll('[data-del]').forEach(b =>
      b.onclick = () => removeEntry(parseInt(b.dataset.del)));
  }

  function renderHint() {
    valueInp.setAttribute('list', addType === 'product' ? 'bl-prod-list'
      : addType === 'category' ? 'bl-cat-list' : '');
    if (addType === 'product') hintEl.textContent = 'Начните вводить — подскажу из каталога.';
    else if (addType === 'category') hintEl.textContent = 'Выберите категорию из списка.';
    else if (addType === 'tag') hintEl.textContent = 'Тег из каталога продукта (например: субпродукты).';
    else hintEl.textContent = 'Подстрока: «свин» заблокирует свинину, фарш свиной, бекон.';
  }

  async function refresh() {
    entries = await invoke('list_food_blacklist').catch(() => []);
    renderSections();
  }

  async function removeEntry(id) {
    await invoke('remove_food_blacklist', { id }).catch(() => {});
    invalidateBlacklistCache();
    await refresh();
    if (onChange) onChange();
  }

  async function addEntry() {
    let value = valueInp.value.trim();
    if (!value) return;
    if (addType === 'category') {
      const code = CAT_ORDER.find(c =>
        CAT_LABELS[c].toLowerCase() === value.toLowerCase() || c === value.toLowerCase());
      if (!code) { hintEl.textContent = 'Неизвестная категория — выберите из списка.'; return; }
      value = code;
    }
    const ok = await applyBlacklist(addType, value, addLevel, null, async () => {
      await refresh();
      if (onChange) onChange();
    });
    if (ok) { valueInp.value = ''; valueInp.focus(); }
  }

  q('.bl-add-btn').onclick = addEntry;
  valueInp.onkeydown = (e) => { if (e.key === 'Enter') addEntry(); };
  q('[data-close]').onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  renderPicks();
  renderSections();
  renderHint();
  document.body.appendChild(overlay);
  valueInp.focus();
}
