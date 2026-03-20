// ── tab-data-memory.js — Memory tab (facts browser, search) ──

import { S, invoke, MEMORY_CATEGORIES } from './state.js';
import { escapeHtml, confirmModal, skeletonPage } from './utils.js';

// ── Memory Tab ──
export async function loadMemoryTab(subTab) {
  const el = document.getElementById('memory-content');
  if (!el) return;
  if (subTab === 'Search') loadMemorySearch(el);
  else loadAllFacts(el);
}

async function loadAllFacts(el) {
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="memory-header">
        <div class="uni-section-header" style="margin:0;flex:1;">Память</div>
        <button class="btn-primary" id="mem-tab-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;">${memories.length} фактов</div>
      <div class="memory-browser" id="memory-all-list">
        ${memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
          <div class="memory-item-actions">
            <button class="memory-item-btn memory-edit-btn" data-medit="${m.id}" title="Редактировать">&#9998;</button>
            <button class="memory-item-btn" data-mdel="${m.id}" title="Удалить">&times;</button>
          </div>
        </div>`).join('')}
      </div>`;

    // Delete handlers
    el.querySelectorAll('[data-mdel]').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.mdel) }).catch(e => console.error('delete_memory error:', e)); loadAllFacts(el); }
      });
    });

    // Edit handlers
    el.querySelectorAll('[data-medit]').forEach(btn => {
      btn.addEventListener('click', () => {
        const id = parseInt(btn.dataset.medit);
        const m = memories.find(x => x.id === id);
        if (!m) return;
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal modal-compact">
          <div class="modal-title">Редактировать факт</div>
          <div class="form-group"><label class="form-label">Категория</label>
            <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
          </div>
          <div class="form-group"><label class="form-label">Ключ</label>
            <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
          </div>
          <div class="form-group"><label class="form-label">Значение</label>
            <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
          </div>
          <div class="modal-actions">
            <button class="btn-secondary mem-cancel">Отмена</button>
            <button class="btn-primary mem-save">Сохранить</button>
          </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
        overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
        overlay.querySelector('.mem-save').onclick = async () => {
          const cat = overlay.querySelector('.memory-edit-cat').value;
          const key = overlay.querySelector('.memory-edit-key').value.trim();
          const val = overlay.querySelector('.memory-edit-val').value.trim();
          if (!key || !val) return;
          try {
            await invoke('update_memory', { id, category: cat, key, value: val });
          } catch (err) { console.error('Memory edit error:', err); }
          overlay.remove();
          loadAllFacts(el);
        };
      });
    });

    // Add button
    document.getElementById('mem-tab-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || key.length < 2 || !val || val.length < 2) return;
        try { await invoke('memory_remember', { category: cat, key, value: val }); } catch (err) { console.error('Memory add error:', err); }
        overlay.remove();
        loadAllFacts(el);
      };
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

export function renderMemoryList(memories, el) {
  const list = document.getElementById('settings-mem-list');
  if (!list) return;
  const countEl = document.getElementById('settings-mem-count');
  if (countEl) countEl.textContent = `${memories.length} фактов`;
  list.innerHTML = memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
    <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
    <span class="memory-item-key">${escapeHtml(m.key)}</span>
    <span class="memory-item-value">${escapeHtml(m.value)}</span>
    <div class="memory-item-actions">
      <button class="memory-item-btn memory-edit-btn" data-edit="${m.id}" title="Редактировать">&#9998;</button>
      <button class="memory-item-btn" data-del="${m.id}" title="Удалить">&times;</button>
    </div>
  </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';

  list.querySelectorAll('[data-del]').forEach(btn => {
    btn.addEventListener('click', async () => {
      if (await confirmModal()) { await invoke('delete_memory', { id: parseInt(btn.dataset.del) }).catch(e => console.error('delete_memory error:', e)); loadMemoryInSettings(el); }
    });
  });

  list.querySelectorAll('[data-edit]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id = parseInt(btn.dataset.edit);
      const m = memories.find(x => x.id === id);
      if (!m) return;
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Редактировать факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Сохранить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-edit-cat').value;
        const key = overlay.querySelector('.memory-edit-key').value.trim();
        const val = overlay.querySelector('.memory-edit-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('update_memory', { id, category: cat, key, value: val });
        } catch (err) { console.error('Failed to update memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });
  });
}

export async function loadMemoryInSettings(el) {
  el.innerHTML = skeletonPage();
  try {
    const memories = await invoke('get_all_memories', { search: null }).catch(() => []);
    el.innerHTML = `
      <div class="memory-header">
        <div class="memory-search-box" style="flex:1;">
          <input class="form-input" id="settings-mem-search" placeholder="Поиск по памяти..." autocomplete="off">
        </div>
        <button class="btn-primary" id="mem-add-btn">+ Добавить</button>
      </div>
      <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;" id="settings-mem-count">${memories.length} фактов</div>
      <div class="memory-browser" id="settings-mem-list"></div>`;

    renderMemoryList(memories, el);

    document.getElementById('mem-add-btn')?.addEventListener('click', () => {
      const overlay = document.createElement('div');
      overlay.className = 'modal-overlay';
      overlay.innerHTML = `<div class="modal modal-compact">
        <div class="modal-title">Новый факт</div>
        <div class="form-group"><label class="form-label">Категория</label>
          <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
        </div>
        <div class="form-group"><label class="form-label">Ключ</label>
          <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
        </div>
        <div class="form-group"><label class="form-label">Значение</label>
          <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
        </div>
        <div class="modal-actions">
          <button class="btn-secondary mem-cancel">Отмена</button>
          <button class="btn-primary mem-save">Добавить</button>
        </div>
      </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
      overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
      overlay.querySelector('.mem-save').onclick = async () => {
        const cat = overlay.querySelector('.memory-add-cat').value;
        const key = overlay.querySelector('.memory-add-key').value.trim();
        const val = overlay.querySelector('.memory-add-val').value.trim();
        if (!key || !val) return;
        try {
          await invoke('memory_remember', { category: cat, key, value: val });
        } catch (err) { console.error('Failed to add memory:', err); }
        overlay.remove();
        loadMemoryInSettings(el);
      };
    });

    let searchTimeout;
    document.getElementById('settings-mem-search')?.addEventListener('input', async (e) => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(async () => {
        const q = e.target.value;
        const results = await invoke('get_all_memories', { search: q || null }).catch(() => []);
        renderMemoryList(results, el);
      }, 300);
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Ошибка: ${e}</div>`; }
}

async function loadMemorySearch(el) {
  el.innerHTML = `
    <div class="uni-section-header">Поиск по памяти</div>
    <div class="memory-search-box" style="margin-bottom:16px;">
      <input class="form-input" id="mem-search-input" placeholder="Поиск..." autocomplete="off">
    </div>
    <div class="memory-browser" id="mem-search-results"></div>`;
  let memSearchTimeout;
  document.getElementById('mem-search-input')?.addEventListener('input', async (e) => {
    clearTimeout(memSearchTimeout);
    memSearchTimeout = setTimeout(async () => {
      const q = e.target.value;
      if (!q || q.length < 2) return;
      try {
        const results = await invoke('get_all_memories', { search: q });
        const list = document.getElementById('mem-search-results');
        if (list) list.innerHTML = results.map(m => `<div class="memory-item">
          <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
          <span class="memory-item-key">${escapeHtml(m.key)}</span>
          <span class="memory-item-value">${escapeHtml(m.value)}</span>
        </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';
      } catch (_) {}
    }, 300);
  });
}
