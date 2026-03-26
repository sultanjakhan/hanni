// ── db-view/unified-layout.js — Unified tab layout with sub-panes ──
// Every data tab gets: [Дашборд] [Таблица] [Цели] [Заметки] [Память]

import { S, invoke, TAB_REGISTRY } from '../state.js';
import { escapeHtml, confirmModal } from '../utils.js';

const SUB_PANES = [
  { id: 'dash',  icon: '📊', label: 'Дашборд' },
  { id: 'table', icon: '📋', label: 'Таблица' },
  { id: 'goals', icon: '🎯', label: 'Цели' },
  { id: 'notes', icon: '📝', label: 'Заметки' },
  { id: 'store', icon: '🧠', label: 'Память' },
];

const TAB_LABELS = {
  work: { name: 'Работа', icon: '💼', desc: 'Проекты и задачи' },
  development: { name: 'Развитие', icon: '🚀', desc: 'Обучение и навыки' },
  home: { name: 'Дом', icon: '🏠', desc: 'Дом и хозяйство' },
  hobbies: { name: 'Хобби', icon: '🎮', desc: 'Медиа и коллекции' },
  sports: { name: 'Спорт', icon: '⚽', desc: 'Тренировки и активность' },
  health: { name: 'Здоровье', icon: '❤️', desc: 'Здоровье и привычки' },
  mindset: { name: 'Мышление', icon: '🧠', desc: 'Дневник, настроение, принципы' },
  food: { name: 'Питание', icon: '🍔', desc: 'Рацион и рецепты' },
  money: { name: 'Финансы', icon: '💰', desc: 'Бюджет и расходы' },
  people: { name: 'Люди', icon: '👥', desc: 'Контакты и связи' },
  calendar: { name: 'Календарь', icon: '📅', desc: 'События и расписание' },
  focus: { name: 'Фокус', icon: '🎯', desc: 'Глубокая работа' },
  projects: { name: 'Проекты', icon: '📁', desc: 'Проекты и их задачи' },
};

// ── Tab metadata persistence ──

async function getTabMeta(tabId) {
  try { const v = await invoke('get_ui_state', { key: `tab_meta_${tabId}` }); return v ? JSON.parse(v) : {}; } catch { return {}; }
}

async function saveTabMeta(tabId, meta) {
  await invoke('set_ui_state', { key: `tab_meta_${tabId}`, value: JSON.stringify(meta) }).catch(() => {});
}

/**
 * Render the unified tab layout into a container.
 */
export async function renderUnifiedLayout(el, tabId, config) {
  const activePane = S._unifiedPane?.[tabId] || 'dash';

  const panes = [...SUB_PANES];
  if (config.renderTracking) panes.splice(2, 0, { id: 'tracking', icon: '📈', label: 'Трекинг' });

  // Tab title header — merge defaults with user overrides
  const defaults = TAB_LABELS[tabId] || { name: config.title || tabId, icon: config.icon || '', desc: config.subtitle || '' };
  const meta = await getTabMeta(tabId);
  const icon = meta.icon || defaults.icon;
  const name = meta.name || defaults.name;
  const desc = meta.desc ?? defaults.desc ?? '';

  const tabsHtml = panes.map(p => {
    const count = config.counts?.[p.id];
    const countHtml = count != null ? `<span class="uni-tab-count">(${count})</span>` : '';
    const cls = p.id === activePane ? ' active' : '';
    return `<div class="uni-tab${cls}" data-pane="${p.id}">${p.label}${countHtml}</div>`;
  }).join('');

  el.innerHTML = `
    <div class="uni-header">
      <span class="uni-header-icon" title="Изменить иконку">${icon}</span>
      <span class="uni-header-name" title="Изменить название">${escapeHtml(name)}</span>
      <div class="uni-header-desc" title="Изменить описание">${desc ? escapeHtml(desc) : '<span style="opacity:0.4">Добавить описание…</span>'}</div>
    </div>
    <div class="uni-tabs">${tabsHtml}</div>
    <div class="uni-content">
      <div class="uni-pane" id="uni-pane-${tabId}"></div>
    </div>`;

  // Wire header editing
  wireHeaderEdit(el, tabId, config, meta, defaults);

  // Wire sub-tab clicks
  el.querySelectorAll('.uni-tab').forEach(tab => {
    tab.addEventListener('click', () => {
      if (!S._unifiedPane) S._unifiedPane = {};
      S._unifiedPane[tabId] = tab.dataset.pane;
      renderUnifiedLayout(el, tabId, config);
    });
  });

  // Render active pane
  const paneEl = el.querySelector(`#uni-pane-${tabId}`);
  if (!paneEl) return;

  switch (activePane) {
    case 'dash':
      if (config.renderDash) await config.renderDash(paneEl);
      else paneEl.innerHTML = renderEmptyState('Дашборд', 'Настройте скиллы и триггеры, чтобы Ханни начала заполнять данные');
      break;
    case 'table':
      if (config.renderTable) await config.renderTable(paneEl);
      else paneEl.innerHTML = renderEmptyState('Таблица', 'Пока нет записей');
      break;
    case 'tracking':
      if (config.renderTracking) await config.renderTracking(paneEl);
      else paneEl.innerHTML = renderEmptyState('Трекинг', 'Скоро здесь появится трекинг');
      break;
    case 'goals':
      await renderGoalsPane(paneEl, tabId, config);
      break;
    case 'notes':
      await renderNotesPane(paneEl, tabId, config);
      break;
    case 'store':
      await renderStorePane(paneEl, tabId, config);
      break;
  }
}

// ── Header inline editing ──

function wireHeaderEdit(el, tabId, config, meta, defaults) {
  const iconEl = el.querySelector('.uni-header-icon');
  const nameEl = el.querySelector('.uni-header-name');
  const descEl = el.querySelector('.uni-header-desc');

  // Icon click → small input popup
  iconEl?.addEventListener('click', () => {
    const popup = document.createElement('div');
    popup.className = 'uni-icon-popup';
    popup.innerHTML = `<input class="uni-icon-input" value="${meta.icon || defaults.icon}" maxlength="2" placeholder="🎯">`;
    const rect = iconEl.getBoundingClientRect();
    popup.style.left = rect.left + 'px';
    popup.style.top = rect.bottom + 4 + 'px';
    document.body.appendChild(popup);
    const input = popup.querySelector('input');
    input.focus();
    input.select();
    const save = async () => {
      const val = input.value.trim();
      if (val && val !== (meta.icon || defaults.icon)) { meta.icon = val; await saveTabMeta(tabId, meta); }
      popup.remove();
      renderUnifiedLayout(el, tabId, config);
    };
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') save(); if (e.key === 'Escape') { popup.remove(); } e.stopPropagation(); });
    input.addEventListener('blur', save);
  });

  // Name click → contenteditable
  nameEl?.addEventListener('click', () => {
    if (nameEl.contentEditable === 'true') return;
    nameEl.contentEditable = 'true';
    nameEl.focus();
    const range = document.createRange();
    range.selectNodeContents(nameEl);
    window.getSelection().removeAllRanges();
    window.getSelection().addRange(range);
    const save = async () => {
      nameEl.contentEditable = 'false';
      const val = nameEl.textContent.trim();
      if (val && val !== (meta.name || defaults.name)) { meta.name = val; await saveTabMeta(tabId, meta); }
    };
    nameEl.addEventListener('blur', save, { once: true });
    nameEl.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); nameEl.blur(); } if (e.key === 'Escape') { nameEl.textContent = meta.name || defaults.name; nameEl.blur(); } e.stopPropagation(); });
  });

  // Desc click → contenteditable
  descEl?.addEventListener('click', () => {
    if (descEl.contentEditable === 'true') return;
    const currentDesc = meta.desc ?? defaults.desc ?? '';
    descEl.innerHTML = escapeHtml(currentDesc);
    descEl.contentEditable = 'true';
    descEl.focus();
    if (currentDesc) {
      const range = document.createRange();
      range.selectNodeContents(descEl);
      window.getSelection().removeAllRanges();
      window.getSelection().addRange(range);
    }
    const save = async () => {
      descEl.contentEditable = 'false';
      const val = descEl.textContent.trim();
      if (val !== (meta.desc ?? defaults.desc ?? '')) { meta.desc = val; await saveTabMeta(tabId, meta); }
      if (!val) descEl.innerHTML = '<span style="opacity:0.4">Добавить описание…</span>';
    };
    descEl.addEventListener('blur', save, { once: true });
    descEl.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); descEl.blur(); } if (e.key === 'Escape') { descEl.textContent = meta.desc ?? defaults.desc ?? ''; descEl.blur(); } e.stopPropagation(); });
  });
}

// ── Goals Pane ──
async function renderGoalsPane(el, tabId, config) {
  if (config.renderGoals) { await config.renderGoals(el); return; }

  let goals = [];
  try { goals = await invoke('get_goals', { tabName: tabId }); } catch {}

  const goalsHtml = goals.length > 0
    ? goals.map(g => {
        const pct = g.target_value > 0 ? Math.min(100, Math.round((g.current_value / g.target_value) * 100)) : 0;
        const color = pct >= 75 ? 'var(--color-green)' : pct >= 40 ? 'var(--color-yellow)' : 'var(--accent-blue)';
        return `<div class="uni-goal">
          <div class="uni-goal-header">
            <div class="uni-goal-title">${escapeHtml(g.title)}</div>
            <div class="uni-goal-pct" style="color:${color}">${pct}%</div>
          </div>
          <div class="uni-goal-bar"><div class="uni-goal-fill" style="width:${pct}%;background:${color}"></div></div>
          <div class="uni-goal-meta">${g.unit ? escapeHtml(g.current_value + '/' + g.target_value + ' ' + g.unit) : ''}${g.deadline ? ' · до ' + g.deadline : ''}</div>
        </div>`;
      }).join('')
    : '<div class="uni-empty">Пока нет целей</div>';

  el.innerHTML = `<div class="uni-goals-list">${goalsHtml}<div class="uni-goal-add" id="uni-add-goal-${tabId}">+ Добавить цель</div></div>`;

  el.querySelector(`#uni-add-goal-${tabId}`)?.addEventListener('click', () => {
    showAddGoalModal(el, tabId, config);
  });
}

function showAddGoalModal(parentEl, tabId, config) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новая цель</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="goal-title" placeholder="Например: Пробежать 100 км"></div>
    <div class="form-group"><label class="form-label">Целевое значение</label><input class="form-input" id="goal-target" type="number" placeholder="100"></div>
    <div class="form-group"><label class="form-label">Единица измерения</label><input class="form-input" id="goal-unit" placeholder="км, книг, часов..."></div>
    <div class="form-group"><label class="form-label">Дедлайн</label><input class="form-input" id="goal-deadline" type="date"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="goal-save">Создать</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('goal-save')?.addEventListener('click', async () => {
    const title = document.getElementById('goal-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_goal', {
        tabName: tabId,
        title,
        targetValue: parseFloat(document.getElementById('goal-target')?.value) || 1,
        unit: document.getElementById('goal-unit')?.value || null,
        deadline: document.getElementById('goal-deadline')?.value || null,
      });
      overlay.remove();
      renderGoalsPane(parentEl, tabId, config);
    } catch (err) { console.error('Error:', err); }
  });
}

// ── Notes Pane ──
async function renderNotesPane(el, tabId, config) {
  if (config.renderNotes) { await config.renderNotes(el); return; }

  let notes = [];
  try { notes = await invoke('get_notes_for_tab', { tabName: tabId }); } catch {}
  if (notes.length === 0) {
    try {
      const all = await invoke('get_notes', { filter: null, search: null });
      notes = (all || []).filter(n => (n.tags || '').toLowerCase().includes(tabId));
    } catch {}
  }

  const notesHtml = notes.length > 0
    ? notes.map(n => `<div class="uni-note-card" data-id="${n.id}">
        ${n.pinned ? '<div class="uni-note-pin">📌</div>' : ''}
        <div class="uni-note-body" ${!n.pinned ? 'style="padding-left:24px"' : ''}>
          <div class="uni-note-title">${escapeHtml(n.title || 'Без названия')}</div>
          ${n.content ? `<div class="uni-note-preview">${escapeHtml(n.content.substring(0, 120))}</div>` : ''}
          <div class="uni-note-meta">
            ${(n.tags || '').split(',').filter(Boolean).map(t => `<span class="uni-note-tag">${escapeHtml(t.trim())}</span>`).join('')}
            <span class="uni-note-date">${n.updated_at ? new Date(n.updated_at).toLocaleDateString('ru') : ''}</span>
          </div>
        </div>
        <div class="uni-note-actions">
          <div class="uni-note-action uni-note-edit" data-id="${n.id}" title="Редактировать">✏️</div>
          <div class="uni-note-action uni-note-archive" data-id="${n.id}" title="Архивировать">📥</div>
        </div>
      </div>`).join('')
    : `<div class="uni-empty">Нет заметок для «${escapeHtml(tabId)}»</div>`;

  el.innerHTML = `<div class="uni-notes-container">
    <div class="uni-notes-toolbar">
      <input class="uni-notes-search" placeholder="Поиск заметок...">
      <button class="uni-notes-add" id="uni-add-note-${tabId}">+ Заметка</button>
    </div>
    <div class="uni-notes-grid">${notesHtml}</div>
  </div>`;

  el.querySelector(`#uni-add-note-${tabId}`)?.addEventListener('click', () => {
    showAddNoteModal(el, tabId, config);
  });

  el.querySelectorAll('.uni-note-edit').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const note = notes.find(n => n.id === parseInt(btn.dataset.id));
      if (note) showEditNoteModal(el, tabId, config, note);
    });
  });

  el.querySelectorAll('.uni-note-archive').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await invoke('update_note', { id: parseInt(btn.dataset.id), title: '', content: '', tags: '', archived: true });
        renderNotesPane(el, tabId, config);
      } catch {}
    });
  });

  el.querySelector('.uni-notes-search')?.addEventListener('input', (e) => {
    const q = e.target.value.toLowerCase();
    el.querySelectorAll('.uni-note-card').forEach(card => {
      card.style.display = card.textContent.toLowerCase().includes(q) ? '' : 'none';
    });
  });
}

function showAddNoteModal(parentEl, tabId, config) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Новая заметка</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="note-title"></div>
    <div class="form-group"><label class="form-label">Содержимое</label><textarea class="form-textarea" id="note-content" rows="5"></textarea></div>
    <div class="form-group"><label class="form-label">Теги</label><input class="form-input" id="note-tags" value="${tabId}" placeholder="через запятую"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="note-save">Создать</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('note-save')?.addEventListener('click', async () => {
    const title = document.getElementById('note-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('create_note', {
        title,
        content: document.getElementById('note-content')?.value || '',
        tags: document.getElementById('note-tags')?.value || tabId,
        tabName: tabId,
        status: null, dueDate: null, reminderAt: null,
      });
      overlay.remove();
      renderNotesPane(parentEl, tabId, config);
    } catch (err) { console.error('Error:', err); }
  });
}

function showEditNoteModal(parentEl, tabId, config, note) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Редактировать заметку</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="note-title" value="${escapeHtml(note.title || '')}"></div>
    <div class="form-group"><label class="form-label">Содержимое</label><textarea class="form-textarea" id="note-content" rows="5">${escapeHtml(note.content || '')}</textarea></div>
    <div class="form-group"><label class="form-label">Теги</label><input class="form-input" id="note-tags" value="${escapeHtml(note.tags || '')}"></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="note-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('note-save')?.addEventListener('click', async () => {
    try {
      await invoke('update_note', {
        id: note.id,
        title: document.getElementById('note-title')?.value || '',
        content: document.getElementById('note-content')?.value || '',
        tags: document.getElementById('note-tags')?.value || '',
        pinned: null, archived: null, tabName: null, status: null, dueDate: null, reminderAt: null, contentBlocks: null,
      });
      overlay.remove();
      renderNotesPane(parentEl, tabId, config);
    } catch (err) { console.error('Error:', err); }
  });
}

// ── Store (Memory) Pane ──
const STORE_HINTS = {
  work: { examples: ['Резюме', 'Портфолио', 'Навыки', 'Сопроводительное письмо'], placeholder: 'резюме, навыки...' },
  health: { examples: ['Лекарства', 'Анализы', 'Диагнозы', 'Врачи'], placeholder: 'лекарства, анализы...' },
  food: { examples: ['Что есть дома', 'Аллергии', 'Любимые продукты'], placeholder: 'продукты, аллергии...' },
  sports: { examples: ['Личные рекорды', 'Программа тренировок', 'Замеры'], placeholder: 'рекорды, замеры...' },
  money: { examples: ['Реквизиты', 'Карты', 'Инвестиции'], placeholder: 'реквизиты, карты...' },
  development: { examples: ['Стек технологий', 'Сертификаты', 'GitHub проекты'], placeholder: 'стек, сертификаты...' },
  mindset: { examples: ['Аффирмации', 'Книги к прочтению', 'Цитаты'], placeholder: 'цитаты, книги...' },
  home: { examples: ['Адреса', 'Контакты ЖКХ', 'Инвентарь'], placeholder: 'адреса, контакты...' },
  hobbies: { examples: ['Вишлист', 'Коллекции', 'Рекомендации'], placeholder: 'вишлист, рекомендации...' },
  people: { examples: ['Дни рождения', 'Подарки', 'Заметки о людях'], placeholder: 'дни рождения, подарки...' },
  calendar: { examples: ['Праздники', 'Расписание', 'Повторяющиеся'], placeholder: 'праздники, расписание...' },
  focus: { examples: ['Правила', 'Блоклист', 'Ритуалы'], placeholder: 'правила, ритуалы...' },
};

async function renderStorePane(el, tabId, config) {
  if (config.renderStore) { await config.renderStore(el); return; }

  let entries = [];
  try {
    entries = await invoke('memory_list', { category: tabId, limit: 100 });
  } catch {}

  const hints = STORE_HINTS[tabId] || { examples: ['Ключ', 'Значение'], placeholder: 'поиск...' };
  const viewMode = S._storeView?.[tabId] || 'table';

  // Cards view
  const cardsHtml = entries.map(e => `<div class="uni-store-card" data-store-id="${e.id}">
      <div class="uni-store-card-key">${escapeHtml(e.key)}</div>
      <div class="uni-store-card-value">${escapeHtml((e.value || '').substring(0, 200))}</div>
      ${e.updated_at ? `<div class="uni-store-card-date">${new Date(e.updated_at).toLocaleDateString('ru')}</div>` : ''}
      <div class="uni-store-card-actions">
        <button class="uni-store-edit-btn" data-store-id="${e.id}" title="Редактировать">✏️</button>
        <button class="uni-store-del-btn" data-store-id="${e.id}" title="Удалить">🗑</button>
      </div>
    </div>`).join('');

  // Table view
  const rowsHtml = entries.map(e => `<tr data-store-id="${e.id}">
      <td class="uni-store-cell-key">${escapeHtml(e.key)}</td>
      <td class="uni-store-cell-value">${escapeHtml((e.value || '').substring(0, 200))}</td>
      <td class="uni-store-cell-date">${e.updated_at ? new Date(e.updated_at).toLocaleDateString('ru') : ''}</td>
      <td class="uni-store-cell-actions">
        <button class="uni-store-edit-btn" data-store-id="${e.id}" title="Редактировать">✏️</button>
        <button class="uni-store-del-btn" data-store-id="${e.id}" title="Удалить">🗑</button>
      </td>
    </tr>`).join('');

  const emptyHtml = entries.length === 0
    ? `<div class="uni-store-empty">
        <div class="uni-store-empty-icon">🧠</div>
        <div class="uni-store-empty-title">Память пуста</div>
        <div class="uni-store-empty-hints">Примеры: ${hints.examples.map(h => `<span class="uni-store-hint">${h}</span>`).join('')}</div>
        <div class="uni-store-empty-desc">Добавьте записи вручную или Ханни заполнит автоматически</div>
      </div>`
    : '';

  el.innerHTML = `<div class="uni-store-container">
    <div class="uni-store-toolbar">
      <input class="uni-store-search" placeholder="Поиск ${hints.placeholder}">
      <div class="uni-store-view-toggle">
        <button class="uni-store-view-btn${viewMode === 'cards' ? ' active' : ''}" data-view="cards" title="Карточки">▦</button>
        <button class="uni-store-view-btn${viewMode === 'table' ? ' active' : ''}" data-view="table" title="Таблица">☰</button>
      </div>
      <button class="uni-store-add" id="uni-add-store-${tabId}">+ Запись</button>
    </div>
    ${emptyHtml}
    ${entries.length > 0 && viewMode === 'cards' ? `<div class="uni-store-cards">${cardsHtml}</div>` : ''}
    ${entries.length > 0 && viewMode === 'table' ? `<div style="overflow-x:auto;">
      <table class="uni-store-table">
        <thead><tr><th>Ключ</th><th>Значение</th><th>Обновлено</th><th></th></tr></thead>
        <tbody>${rowsHtml}</tbody>
      </table>
    </div>` : ''}
  </div>`;

  // View toggle
  el.querySelectorAll('.uni-store-view-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      if (!S._storeView) S._storeView = {};
      S._storeView[tabId] = btn.dataset.view;
      renderStorePane(el, tabId, config);
    });
  });

  // Add entry
  el.querySelector(`#uni-add-store-${tabId}`)?.addEventListener('click', () => {
    showAddStoreModal(el, tabId, config);
  });

  // Edit entry
  el.querySelectorAll('.uni-store-edit-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const entry = entries.find(en => en.id === parseInt(btn.dataset.storeId));
      if (entry) showEditStoreModal(el, tabId, config, entry);
    });
  });

  // Delete entry
  el.querySelectorAll('.uni-store-del-btn').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const entry = entries.find(en => en.id === parseInt(btn.dataset.storeId));
      if (!entry) return;
      if (!(await confirmModal(`Удалить «${entry.key}»?`))) return;
      try {
        await invoke('memory_forget', { category: tabId, key: entry.key });
        renderStorePane(el, tabId, config);
      } catch (err) { console.error('Error:', err); }
    });
  });

  // Search filter
  el.querySelector('.uni-store-search')?.addEventListener('input', (e) => {
    const q = e.target.value.toLowerCase();
    if (viewMode === 'table') {
      el.querySelectorAll('.uni-store-table tbody tr').forEach(row => {
        row.style.display = row.textContent.toLowerCase().includes(q) ? '' : 'none';
      });
    } else {
      el.querySelectorAll('.uni-store-card').forEach(card => {
        card.style.display = card.textContent.toLowerCase().includes(q) ? '' : 'none';
      });
    }
  });
}

function showAddStoreModal(parentEl, tabId, config) {
  const hints = STORE_HINTS[tabId] || { examples: [] };
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Добавить в память</div>
    <div class="form-group"><label class="form-label">Ключ</label><input class="form-input" id="store-key" placeholder="${hints.examples[0] || 'Название'}"></div>
    <div class="form-group"><label class="form-label">Значение</label><textarea class="form-textarea" id="store-val" rows="4" placeholder="Содержимое..."></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="store-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('store-save')?.addEventListener('click', async () => {
    const key = document.getElementById('store-key')?.value?.trim();
    if (!key) return;
    try {
      await invoke('memory_remember', {
        category: tabId,
        key,
        value: document.getElementById('store-val')?.value || '',
      });
      overlay.remove();
      renderStorePane(parentEl, tabId, config);
    } catch (err) { console.error('Error:', err); }
  });
}

function showEditStoreModal(parentEl, tabId, config, entry) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">Редактировать запись</div>
    <div class="form-group"><label class="form-label">Ключ</label><input class="form-input" id="store-key" value="${escapeHtml(entry.key)}" readonly style="opacity:0.6"></div>
    <div class="form-group"><label class="form-label">Значение</label><textarea class="form-textarea" id="store-val" rows="4">${escapeHtml(entry.value || '')}</textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Отмена</button>
      <button class="btn-primary" id="store-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('store-save')?.addEventListener('click', async () => {
    try {
      await invoke('memory_remember', {
        category: tabId,
        key: entry.key,
        value: document.getElementById('store-val')?.value || '',
      });
      overlay.remove();
      renderStorePane(parentEl, tabId, config);
    } catch (err) { console.error('Error:', err); }
  });
}

// ── Empty state helper ──
function renderEmptyState(title, desc) {
  return `<div class="uni-empty-state">
    <div class="uni-empty-icon">📭</div>
    <div class="uni-empty-title">${escapeHtml(title)}</div>
    <div class="uni-empty-desc">${escapeHtml(desc)}</div>
  </div>`;
}
