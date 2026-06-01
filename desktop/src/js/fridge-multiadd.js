// fridge-multiadd.js — bulk "quick fill" for the fridge. Paste/type several
// products (one per line, e.g. "молоко 2 л") → parse each into {name, qty, unit}
// → add them all in one go. Loaded on demand from fridge-shared.js; the button
// is Hanni-only so the guest UI never imports this module.

const UNITS = ['шт', 'г', 'кг', 'мл', 'л', 'упак.'];
const UNIT_KEYS = UNITS.map(u => u.replace(/\.$/, ''));
const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

// "молоко 2 л" → {name:'молоко', quantity:2, unit:'л'} · "хлеб" → qty 1, шт.
function parseLine(line) {
  const t = String(line || '').trim();
  if (!t) return null;
  const m = t.match(/^(.*?)[\s,]+([\d.,]+)\s*([^\d\s]*)\.?\s*$/);
  if (m && m[1].trim()) {
    const qty = parseFloat(m[2].replace(',', '.')) || 1;
    const u = (m[3] || '').trim().toLowerCase();
    const unit = UNIT_KEYS.includes(u) ? (UNITS.find(x => x.replace(/\.$/, '') === u) || u) : (u || 'шт');
    return { name: m[1].trim(), quantity: qty, unit };
  }
  return { name: t, quantity: 1, unit: 'шт' };
}

export async function showMultiAddModal({ backend, location = 'fridge', onAdded }) {
  let catalog = [];
  try { catalog = (backend.getCatalog ? await backend.getCatalog() : []) || []; } catch {}
  const catByName = new Map(catalog.map(c => [String(c.name).trim().toLowerCase(), c]));

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:460px">
    <div class="modal-title">📋 Быстрое добавление</div>
    <div class="form-group"><label class="form-label">По одному продукту в строке</label>
      <textarea class="form-textarea" id="ma-text" rows="6" placeholder="молоко 2 л&#10;курица 1 кг&#10;яблоки 6 шт"></textarea></div>
    <div id="ma-preview" style="font-size:12px;color:var(--text-muted);min-height:18px;margin-bottom:8px"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="ma-cancel">Отмена</button>
      <button class="btn-primary" id="ma-add" disabled>Добавить</button>
    </div></div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('#ma-cancel').onclick = close;

  const ta = overlay.querySelector('#ma-text');
  const prev = overlay.querySelector('#ma-preview');
  const addBtn = overlay.querySelector('#ma-add');
  let parsed = [];
  function refresh() {
    parsed = ta.value.split('\n').map(parseLine).filter(Boolean);
    prev.innerHTML = parsed.length
      ? `Распознано: ${parsed.length} · ` + parsed.slice(0, 4).map(p => `${esc(p.name)} (${p.quantity} ${esc(p.unit)})`).join(', ') + (parsed.length > 4 ? '…' : '')
      : '';
    addBtn.disabled = !parsed.length;
    addBtn.textContent = parsed.length ? `Добавить ${parsed.length}` : 'Добавить';
  }
  ta.oninput = refresh;

  addBtn.onclick = async () => {
    addBtn.disabled = true; addBtn.textContent = 'Добавляю…';
    for (const p of parsed) {
      const cat = catByName.get(p.name.toLowerCase());
      try {
        await backend.add({
          name: p.name, category: cat ? cat.category : 'other',
          quantity: p.quantity, unit: p.unit, expiry_date: null,
          location, notes: '', catalog_id: cat ? cat.id : null,
        });
      } catch (e) { /* skip a failed line, keep going */ }
    }
    close();
    if (onAdded) await onAdded();
  };
  refresh();
  ta.focus();
}
