// ── food-blacklist-modal.js — Manage food blacklist (tags / products / categories / keywords) ──
import { invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { CAT_LABELS, CAT_ORDER, loadCatalog, invalidateBlacklistCache } from './food-recipe-filters.js';

const TYPE_LABELS = { tag: 'Тег', product: 'Продукт', category: 'Категория', keyword: 'Ключевое слово' };

function groupEntries(entries) {
  const groups = { tag: [], product: [], category: [], keyword: [] };
  for (const e of entries) (groups[e.type] ||= []).push(e);
  return groups;
}

export async function showBlacklistModal(onChange) {
  const [entries, catalog] = await Promise.all([
    invoke('list_food_blacklist').catch(() => []),
    loadCatalog(),
  ]);

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:520px">
    <div class="modal-title">🚫 Блэклист еды</div>
    <p style="color:var(--text-secondary);font-size:13px;margin:0 0 12px;">Заблокированные элементы не показываются в рецептах и автокомплите. При добавлении существующие рецепты удаляются.</p>
    <div class="bl-list"></div>
    <div class="bl-add" style="margin-top:16px;padding-top:12px;border-top:1px solid var(--border-color)">
      <div class="settings-row" style="display:flex;gap:8px;align-items:center;margin-bottom:8px">
        <select class="form-input bl-type" style="width:140px">
          ${Object.entries(TYPE_LABELS).map(([k, v]) => `<option value="${k}">${v}</option>`).join('')}
        </select>
        <input class="form-input bl-value" placeholder="Значение..." style="flex:1">
        <button class="btn-primary bl-add-btn" style="font-size:13px;padding:6px 14px">Добавить</button>
      </div>
      <div class="bl-hint" style="color:var(--text-muted);font-size:12px"></div>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" data-close>Закрыть</button>
    </div>
  </div>`;

  const listEl = overlay.querySelector('.bl-list');
  const typeSel = overlay.querySelector('.bl-type');
  const valueInp = overlay.querySelector('.bl-value');
  const hintEl = overlay.querySelector('.bl-hint');

  function renderList() {
    const groups = groupEntries(entries);
    if (!entries.length) { listEl.innerHTML = '<div class="empty-state" style="padding:16px">Блэклист пуст</div>'; return; }
    listEl.innerHTML = Object.entries(groups).filter(([_, arr]) => arr.length).map(([type, arr]) =>
      `<div class="bl-group" style="margin-bottom:10px">
        <div style="font-size:12px;color:var(--text-muted);margin-bottom:4px">${TYPE_LABELS[type]}</div>
        <div style="display:flex;flex-wrap:wrap;gap:6px">
          ${arr.map(e => `<span class="rf-chip active" style="display:inline-flex;align-items:center;gap:6px">
            ${escapeHtml(e.value)}
            <button data-del="${e.id}" style="background:none;border:none;color:inherit;cursor:pointer;padding:0 2px;font-size:14px">×</button>
          </span>`).join('')}
        </div>
      </div>`
    ).join('');
    listEl.querySelectorAll('[data-del]').forEach(btn => btn.onclick = () => removeEntry(parseInt(btn.dataset.del)));
  }

  function renderHint() {
    const type = typeSel.value;
    if (type === 'tag') {
      const tags = [...new Set(catalog.flatMap(c => (c.tags || '').split(',').map(t => t.trim()).filter(Boolean)))].sort();
      hintEl.textContent = `Теги из каталога: ${tags.slice(0, 12).join(', ')}${tags.length > 12 ? '...' : ''}`;
    } else if (type === 'category') {
      hintEl.textContent = 'Категории: ' + CAT_ORDER.map(c => `${c} (${CAT_LABELS[c]})`).join(', ');
    } else if (type === 'product') {
      hintEl.textContent = 'Название продукта из каталога (например: «кефир»).';
    } else {
      hintEl.textContent = 'Любая подстрока (например: «свин» — заблокирует «свинина», «фарш свиной», «бекон»).';
    }
  }

  async function removeEntry(id) {
    if (!await confirmModal('Убрать из блэклиста?', 'Убрать')) return;
    await invoke('remove_food_blacklist', { id });
    const idx = entries.findIndex(e => e.id === id);
    if (idx >= 0) entries.splice(idx, 1);
    invalidateBlacklistCache();
    renderList();
    if (onChange) onChange();
  }

  async function addEntry() {
    const value = valueInp.value.trim();
    if (!value) return;
    const type = typeSel.value;
    const hits = await invoke('find_recipes_matching_blacklist', { entryType: type, value }).catch(() => []);
    const preview = hits.length ? `\n\nБудет удалено ${hits.length} рецептов:\n• ${hits.slice(0, 8).map(h => h.name).join('\n• ')}${hits.length > 8 ? '\n• ...' : ''}` : '';
    if (!await confirmModal(`Заблокировать «${value}»?${preview}`, 'Заблокировать')) return;
    const id = await invoke('add_food_blacklist', { entryType: type, value });
    if (hits.length) await invoke('delete_recipes_matching_blacklist', { entryType: type, value });
    entries.push({ id, type, value: value.toLowerCase() });
    valueInp.value = '';
    invalidateBlacklistCache();
    renderList();
    if (onChange) onChange();
  }

  overlay.querySelector('.bl-add-btn').onclick = addEntry;
  valueInp.onkeydown = (e) => { if (e.key === 'Enter') addEntry(); };
  typeSel.onchange = renderHint;
  overlay.querySelector('[data-close]').onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  renderList();
  renderHint();
  document.body.appendChild(overlay);
  valueInp.focus();
}
