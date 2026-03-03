// ── js/tab-notes.js — Notes tab + Database View system (Notion-style) ──

import { S, invoke, tabLoaders, TAB_REGISTRY, PROPERTY_TYPE_DEFS, getTypeIcon, getTypeName } from './state.js';
import { escapeHtml, renderPageHeader, setupPageHeaderControls, confirmModal, skeletonPage, initBlockEditor, blocksToPlainText, migrateTextToBlocks } from './utils.js';
import { openTab } from './tabs.js';

// ── Helpers ──

function showStub(containerId, icon, label, desc) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">${icon}</div><div class="empty-state-text">${label}</div><div class="empty-state-sub">${desc}</div></div>`;
}

// ── Notes ──

function formatNoteDate(dateStr) {
  const date = new Date(dateStr);
  const now = new Date();
  const diff = now - date;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'только что';
  if (mins < 60) return `${mins} мин назад`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} ч назад`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} дн назад`;
  return date.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
}

function formatDueDate(dateStr) {
  if (!dateStr) return '';
  const due = new Date(dateStr + (dateStr.includes('T') ? '' : 'T23:59:59'));
  const now = new Date();
  const diffMs = due - now;
  const days = Math.ceil(diffMs / 86400000);
  if (days < 0) return `<span class="due-overdue">просрочено ${Math.abs(days)} дн</span>`;
  if (days === 0) return '<span class="due-today">сегодня</span>';
  if (days <= 3) return `<span class="due-soon">через ${days} дн</span>`;
  return `<span class="due-later">через ${days} дн</span>`;
}

function renderNoteTags(tagsStr) {
  if (!tagsStr) return '';
  return tagsStr.split(',').map(t => t.trim()).filter(Boolean).map(t => {
    const color = S.tagColorMap[t] || 'blue';
    return `<span class="note-tag badge-${color}">${escapeHtml(t)}</span>`;
  }).join('');
}

async function loadTagColorMap() {
  try {
    const tags = await invoke('get_note_tags');
    S.tagColorMap = {};
    for (const t of tags) S.tagColorMap[t.name] = t.color;
  } catch (_) {}
}

async function loadNotes(subTab) {
  await renderNotesPage();
}

// ── Notes Page: Notion-like views + filter chips ──

function applyNotesFilters(notes) {
  let result = notes;

  // Status filters (OR logic)
  if (S.notesFilters.size > 0) {
    const todayStr = new Date().toISOString().slice(0, 10);
    result = result.filter(n => {
      const conditions = [];
      if (S.notesFilters.has('pin')) conditions.push(!!n.pinned);
      if (S.notesFilters.has('archive')) conditions.push(!!n.archived);
      if (S.notesFilters.has('task')) conditions.push(n.status === 'task');
      if (S.notesFilters.has('done')) conditions.push(n.status === 'done');
      if (S.notesFilters.has('overdue')) conditions.push(n.status === 'task' && n.due_date && n.due_date < todayStr);
      return conditions.some(c => c);
    });
  } else {
    result = result.filter(n => !n.archived);
  }

  // Tag filter (AND)
  if (S.noteTagFilter) {
    result = result.filter(n => (n.tags || '').split(',').map(t => t.trim()).includes(S.noteTagFilter));
  }

  // Search filter
  if (S.notesSearchQuery) {
    const q = S.notesSearchQuery.toLowerCase();
    result = result.filter(n =>
      (n.title || '').toLowerCase().includes(q) ||
      (n.content || '').toLowerCase().includes(q) ||
      (n.tags || '').toLowerCase().includes(q)
    );
  }

  return result;
}

async function renderNotesPage() {
  const el = document.getElementById('notes-content');
  if (!el) return;

  if (S.notesViewMode === 'edit' && S.currentNoteId) {
    renderNoteEditor(el, S.currentNoteId);
    return;
  }

  S.notesViewMode = 'list';
  await loadTagColorMap();

  let allNotes;
  try {
    allNotes = await invoke('get_notes', { filter: null, search: null }) || [];
  } catch (e) {
    showStub('notes-content', '📝', 'Заметки', 'Быстрые заметки и мысли');
    return;
  }

  const filtered = applyNotesFilters(allNotes);

  // Collect all tags
  const allTags = new Set();
  allNotes.forEach(n => (n.tags || '').split(',').map(t => t.trim()).filter(Boolean).forEach(t => allTags.add(t)));

  el.innerHTML = renderPageHeader('notes') + renderNotesToolbar() + renderNotesViewBar() + renderNotesFilterBar(allTags) + `<div id="notes-view-content" class="page-content"></div>`;

  const content = document.getElementById('notes-view-content');
  if (!content) return;

  switch (S.notesView) {
    case 'kanban':   renderKanbanContent(content, filtered); break;
    case 'timeline': renderTimelineContent(content, filtered); break;
    case 'table':    renderTableView(content, filtered); break;
    case 'gallery':  renderGalleryView(content, filtered); break;
    default:         renderListContent(content, filtered, allNotes); break;
  }

  setupNotesControls();
}

function renderNotesToolbar() {
  return `<div class="notes-toolbar">
    <button class="btn-primary" id="new-note-btn">+ Новая заметка</button>
    <div class="notes-search-wrap">
      <input class="form-input" id="notes-search" placeholder="Поиск..." autocomplete="off" value="${escapeHtml(S.notesSearchQuery)}">
    </div>
  </div>`;
}

function renderNotesViewBar() {
  const views = [
    { id: 'all', label: 'All' },
    { id: 'kanban', label: 'Kanban' },
    { id: 'timeline', label: 'Timeline' },
    { id: 'table', label: 'Table' },
    { id: 'gallery', label: 'Gallery' },
  ];
  return `<div class="notes-view-bar">
    ${views.map(v => `<button class="notes-view-btn${S.notesView === v.id ? ' active' : ''}" data-view="${v.id}">${v.label}</button>`).join('')}
  </div>`;
}

function renderNotesFilterBar(allTags) {
  const filters = [
    { id: 'pin', label: '📌 Pin' },
    { id: 'archive', label: '📦 Архив' },
    { id: 'task', label: '☐ Задачи' },
    { id: 'done', label: '✅ Готово' },
    { id: 'overdue', label: '🔴 Просрочено' },
  ];
  const tagChips = allTags.size > 0 ? `<span class="notes-filter-divider"></span>` +
    [...allTags].map(t => `<button class="notes-filter-chip tag badge-${S.tagColorMap[t] || 'blue'}${S.noteTagFilter === t ? ' active' : ''}" data-tag="${escapeHtml(t)}">${escapeHtml(t)}</button>`).join('') : '';

  return `<div class="notes-filter-bar">
    ${filters.map(f => `<button class="notes-filter-chip${S.notesFilters.has(f.id) ? ' active' : ''}" data-filter="${f.id}">${f.label}</button>`).join('')}
    ${tagChips}
  </div>`;
}

function setupNotesControls() {
  // Page header controls (icon picker, description)
  setupPageHeaderControls('notes');

  // View bar clicks
  document.querySelectorAll('.notes-view-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      S.notesView = btn.dataset.view;
      localStorage.setItem('hanni_notes_view', S.notesView);
      renderNotesPage();
    });
  });

  // Filter chip clicks
  document.querySelectorAll('.notes-filter-chip[data-filter]').forEach(chip => {
    chip.addEventListener('click', () => {
      const f = chip.dataset.filter;
      if (S.notesFilters.has(f)) S.notesFilters.delete(f);
      else S.notesFilters.add(f);
      renderNotesPage();
    });
  });

  // Tag chip clicks
  document.querySelectorAll('.notes-filter-chip[data-tag]').forEach(chip => {
    chip.addEventListener('click', () => {
      const t = chip.dataset.tag;
      S.noteTagFilter = S.noteTagFilter === t ? null : t;
      renderNotesPage();
    });
  });

  // New note
  document.getElementById('new-note-btn')?.addEventListener('click', createAndOpenNote);

  // Search
  document.getElementById('notes-search')?.addEventListener('input', (e) => {
    clearTimeout(S.noteAutoSaveTimeout);
    S.noteAutoSaveTimeout = setTimeout(() => {
      S.notesSearchQuery = e.target.value || '';
      renderNotesPage();
    }, 300);
  });
}

// ── List View (default) ──
function renderListContent(container, notes, allNotes) {
  const pinned = notes.filter(n => n.pinned && !n.archived);
  const regular = notes.filter(n => !n.pinned);

  if (notes.length === 0) {
    container.innerHTML = `<div class="empty-state">
      <div class="empty-state-icon">📝</div>
      <div class="empty-state-text">Нет заметок</div>
      <button class="btn-primary" id="empty-new-note-btn">Создать первую</button>
    </div>`;
    document.getElementById('empty-new-note-btn')?.addEventListener('click', createAndOpenNote);
    return;
  }

  const list = document.createElement('div');
  list.className = 'notes-card-list';
  list.id = 'notes-card-list';
  const refresh = () => renderNotesPage();

  if (pinned.length > 0) {
    const section = document.createElement('div');
    section.className = 'notes-section-label';
    section.textContent = '📌 Закреплённые';
    list.appendChild(section);
    for (const note of pinned) list.appendChild(createNoteCard(note, refresh));

    if (regular.length > 0) {
      const sep = document.createElement('div');
      sep.className = 'notes-section-label';
      sep.textContent = 'Все заметки';
      list.appendChild(sep);
    }
  }
  for (const note of regular) list.appendChild(createNoteCard(note, refresh));

  container.appendChild(list);
  setupNotesDnD(list);
}

// ── Kanban View ──
function renderKanbanContent(container, notes) {
  const columns = [
    { status: 'note', label: 'Заметки', icon: '📝', items: notes.filter(n => !n.status || n.status === 'note') },
    { status: 'task', label: 'Задачи', icon: '☐', items: notes.filter(n => n.status === 'task') },
    { status: 'done', label: 'Готово', icon: '✅', items: notes.filter(n => n.status === 'done') },
  ];

  container.innerHTML = `<div class="kanban-board" id="kanban-board">
    ${columns.map(col => `
      <div class="kanban-column" data-status="${col.status}">
        <div class="kanban-column-header">
          <span>${col.icon} ${col.label}</span>
          <span class="kanban-column-count">${col.items.length}</span>
          <button class="kanban-add-btn" data-status="${col.status}">+</button>
        </div>
        <div class="kanban-column-cards" data-status="${col.status}"></div>
      </div>
    `).join('')}
  </div>`;

  const refresh = () => renderNotesPage();
  for (const col of columns) {
    const colEl = container.querySelector(`.kanban-column-cards[data-status="${col.status}"]`);
    if (!colEl) continue;
    for (const note of col.items) colEl.appendChild(createNoteCard(note, refresh));
  }

  setupKanbanDnD(container.querySelector('#kanban-board'), refresh);

  container.querySelectorAll('.kanban-add-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      const status = btn.dataset.status === 'note' ? null : btn.dataset.status;
      try {
        const id = await invoke('create_note', { title: '', content: '', tags: '', status, tabName: null, dueDate: null, reminderAt: null });
        S.currentNoteId = id;
        S.notesViewMode = 'edit';
        const el = document.getElementById('notes-content');
        if (el) renderNoteEditor(el, id);
      } catch (err) { console.error('kanban create:', err); }
    });
  });
}

function setupKanbanDnD(board, refresh) {
  if (!board) return;
  board.querySelectorAll('.kanban-column-cards').forEach(col => {
    col.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      col.classList.add('drag-over');
    });
    col.addEventListener('dragleave', (e) => {
      if (!col.contains(e.relatedTarget)) col.classList.remove('drag-over');
    });
    col.addEventListener('drop', async (e) => {
      e.preventDefault();
      e.stopPropagation();
      col.classList.remove('drag-over');
      const raw = e.dataTransfer.getData('text/plain');
      const noteId = parseInt(raw);
      if (!noteId || isNaN(noteId)) return;
      const targetStatus = col.dataset.status;
      try {
        await invoke('update_note_status', { id: noteId, status: targetStatus });
        refresh();
      } catch (err) { console.error('kanban drop:', err); }
    });
  });
}

// ── Timeline View ──
function renderTimelineContent(container, notes) {
  const now = new Date();
  const todayStr = now.toISOString().slice(0, 10);

  const dayOfWeek = now.getDay() || 7;
  const thisMonday = new Date(now);
  thisMonday.setDate(now.getDate() - dayOfWeek + 1);
  thisMonday.setHours(0, 0, 0, 0);
  const thisSunday = new Date(thisMonday);
  thisSunday.setDate(thisMonday.getDate() + 6);
  const nextMonday = new Date(thisMonday);
  nextMonday.setDate(thisMonday.getDate() + 7);
  const nextSunday = new Date(nextMonday);
  nextSunday.setDate(nextMonday.getDate() + 6);

  const toDateStr = (d) => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
  const thisMondayStr = toDateStr(thisMonday);
  const thisSundayStr = toDateStr(thisSunday);
  const nextMondayStr = toDateStr(nextMonday);
  const nextSundayStr = toDateStr(nextSunday);

  const overdue = notes.filter(n => n.status === 'task' && n.due_date && n.due_date < todayStr);
  const thisWeek = notes.filter(n => n.due_date && n.due_date >= thisMondayStr && n.due_date <= thisSundayStr && !(n.status === 'task' && n.due_date < todayStr));
  const nextWeek = notes.filter(n => n.due_date && n.due_date >= nextMondayStr && n.due_date <= nextSundayStr);
  const later = notes.filter(n => n.due_date && n.due_date > nextSundayStr);
  const noDate = notes.filter(n => !n.due_date);

  const formatWeekRange = (mon, sun) => {
    const months = ['янв', 'фев', 'мар', 'апр', 'мая', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
    return `${mon.getDate()} ${months[mon.getMonth()]} – ${sun.getDate()} ${months[sun.getMonth()]}`;
  };

  const groups = [
    { label: 'Просроченные', items: overdue, cls: 'overdue' },
    { label: `Эта неделя (${formatWeekRange(thisMonday, thisSunday)})`, items: thisWeek, cls: '' },
    { label: `Следующая неделя (${formatWeekRange(nextMonday, nextSunday)})`, items: nextWeek, cls: '' },
    { label: 'Позже', items: later, cls: '' },
    { label: 'Без даты', items: noDate, cls: 'no-date' },
  ];

  const months = ['янв', 'фев', 'мар', 'апр', 'мая', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
  const formatDateShort = (dateStr) => {
    if (!dateStr) return '';
    const d = new Date(dateStr + 'T00:00:00');
    return `${d.getDate()} ${months[d.getMonth()]}`;
  };

  const renderItem = (note) => {
    const isDone = note.status === 'done';
    const isOverdue = note.status === 'task' && note.due_date && note.due_date < todayStr;
    const isToday = note.due_date === todayStr;
    const tagsHtml = renderNoteTags(note.tags);
    const dotCls = isDone ? 'done' : isOverdue ? 'overdue' : '';
    const todayBadge = isToday ? '<span class="timeline-today-badge">сегодня</span>' : '';
    const overdueBadge = isOverdue ? `<span class="timeline-overdue-badge">просрочено</span>` : '';
    return `<div class="timeline-item${isDone ? ' task-done' : ''}" data-note-id="${note.id}">
      <div class="timeline-date">${formatDateShort(note.due_date)}</div>
      <div class="timeline-dot ${dotCls}"></div>
      <div class="timeline-content">
        <div class="timeline-title">${escapeHtml(note.title || 'Без названия')}</div>
        ${tagsHtml ? `<div class="timeline-tags">${tagsHtml}</div>` : ''}
      </div>
      ${todayBadge}${overdueBadge}
    </div>`;
  };

  const groupsHtml = groups
    .filter(g => g.items.length > 0)
    .map(g => `<div class="timeline-group ${g.cls}">
      <div class="timeline-group-header">${g.label}</div>
      ${g.items.map(renderItem).join('')}
    </div>`).join('');

  container.innerHTML = `<div class="timeline-view">${groupsHtml || '<div class="empty-state"><div class="empty-state-icon">📅</div><div class="empty-state-text">Нет задач</div></div>'}</div>`;

  container.querySelectorAll('.timeline-item').forEach(item => {
    item.addEventListener('click', () => {
      const noteId = parseInt(item.dataset.noteId);
      if (!noteId) return;
      S.currentNoteId = noteId;
      S.notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

// ── Table View ──
function renderTableView(container, notes) {
  const sortedNotes = [...notes].sort((a, b) => {
    const col = S.notesTableSort.col;
    const dir = S.notesTableSort.dir === 'asc' ? 1 : -1;
    const av = a[col] || '';
    const bv = b[col] || '';
    if (av < bv) return -1 * dir;
    if (av > bv) return 1 * dir;
    return 0;
  });

  const sortIcon = (col) => {
    if (S.notesTableSort.col !== col) return '';
    return S.notesTableSort.dir === 'asc' ? ' &uarr;' : ' &darr;';
  };

  const statusPill = (status) => {
    if (status === 'done') return '<span class="table-status-pill table-status-done">Готово</span>';
    if (status === 'task') return '<span class="table-status-pill table-status-task">Задача</span>';
    return '<span class="table-status-pill table-status-note">Заметка</span>';
  };

  const rows = sortedNotes.map(n => {
    const tagsHtml = (n.tags || '').split(',').map(t => t.trim()).filter(Boolean)
      .map(t => `<span class="note-tag badge-${S.tagColorMap[t] || 'blue'}">${escapeHtml(t)}</span>`).join('');
    const dueHtml = n.due_date ? formatDueDate(n.due_date) : '<span class="text-faint">—</span>';
    return `<tr class="notes-table-row" data-note-id="${n.id}">
      <td class="notes-table-title">${n.pinned ? '📌 ' : ''}${escapeHtml(n.title || 'Без названия')}</td>
      <td>${statusPill(n.status)}</td>
      <td>${dueHtml}</td>
      <td class="notes-table-tags">${tagsHtml || '<span class="text-faint">—</span>'}</td>
      <td class="text-faint">${formatNoteDate(n.updated_at || n.created_at)}</td>
    </tr>`;
  }).join('');

  container.innerHTML = `<div class="notes-table-wrap">
    <table class="notes-table">
      <thead>
        <tr>
          <th class="notes-table-sortable" data-sort="title">Название${sortIcon('title')}</th>
          <th class="notes-table-sortable" data-sort="status">Статус${sortIcon('status')}</th>
          <th class="notes-table-sortable" data-sort="due_date">Дата${sortIcon('due_date')}</th>
          <th>Теги</th>
          <th class="notes-table-sortable" data-sort="updated_at">Обновлено${sortIcon('updated_at')}</th>
        </tr>
      </thead>
      <tbody>${rows || '<tr><td colspan="5"><div class="empty-state"><div class="empty-state-icon">📋</div><div class="empty-state-text">Нет заметок</div></div></td></tr>'}</tbody>
    </table>
  </div>`;

  // Sort clicks
  container.querySelectorAll('.notes-table-sortable').forEach(th => {
    th.addEventListener('click', () => {
      const col = th.dataset.sort;
      if (S.notesTableSort.col === col) {
        S.notesTableSort.dir = S.notesTableSort.dir === 'asc' ? 'desc' : 'asc';
      } else {
        S.notesTableSort.col = col;
        S.notesTableSort.dir = 'asc';
      }
      renderNotesPage();
    });
  });

  // Row clicks
  container.querySelectorAll('.notes-table-row').forEach(row => {
    row.addEventListener('click', () => {
      const noteId = parseInt(row.dataset.noteId);
      if (!noteId) return;
      S.currentNoteId = noteId;
      S.notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

// ── Gallery View ──
function renderGalleryView(container, notes) {
  if (notes.length === 0) {
    container.innerHTML = `<div class="empty-state">
      <div class="empty-state-icon">🖼</div>
      <div class="empty-state-text">Нет заметок</div>
      <button class="btn-primary" id="empty-new-note-btn">Создать первую</button>
    </div>`;
    document.getElementById('empty-new-note-btn')?.addEventListener('click', createAndOpenNote);
    return;
  }

  const cards = notes.map(n => {
    const preview = (n.content || '').substring(0, 200).replace(/\n/g, ' ');
    const statusIcon = n.status === 'done' ? '☑ ' : n.status === 'task' ? '☐ ' : '';
    const tagsHtml = (n.tags || '').split(',').map(t => t.trim()).filter(Boolean)
      .map(t => `<span class="note-tag badge-${S.tagColorMap[t] || 'blue'}">${escapeHtml(t)}</span>`).join('');
    const dueHtml = (n.status === 'task' || n.status === 'done') && n.due_date ? `<span class="gallery-card-due">${formatDueDate(n.due_date)}</span>` : '';
    return `<div class="gallery-card card${n.status === 'done' ? ' task-done' : ''}" data-note-id="${n.id}">
      <div class="gallery-card-header">
        <span class="gallery-card-title">${statusIcon}${n.pinned ? '📌 ' : ''}${escapeHtml(n.title || 'Без названия')}</span>
      </div>
      ${preview ? `<div class="gallery-card-preview">${escapeHtml(preview)}</div>` : ''}
      <div class="gallery-card-footer">
        ${tagsHtml ? `<div class="gallery-card-tags">${tagsHtml}</div>` : ''}
        <div class="gallery-card-meta">
          <span class="text-faint">${formatNoteDate(n.updated_at || n.created_at)}</span>
          ${dueHtml}
        </div>
      </div>
    </div>`;
  }).join('');

  container.innerHTML = `<div class="gallery-grid">${cards}</div>`;

  container.querySelectorAll('.gallery-card').forEach(card => {
    card.addEventListener('click', () => {
      const noteId = parseInt(card.dataset.noteId);
      if (!noteId) return;
      S.currentNoteId = noteId;
      S.notesViewMode = 'edit';
      const el = document.getElementById('notes-content');
      if (el) renderNoteEditor(el, noteId);
    });
  });
}

async function createAndOpenTask() {
  try {
    const id = await invoke('create_note', { title: '', content: '', tags: '', status: 'task', tabName: null, dueDate: null, reminderAt: null });
    S.currentNoteId = id;
    S.notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, id);
  } catch (err) { console.error('create_task error:', err); }
}

function createNoteCard(note, onRefresh) {
  const card = document.createElement('div');
  const isTask = note.status === 'task' || note.status === 'done';
  card.className = `note-card card${note.status === 'done' ? ' task-done' : ''}`;
  card.draggable = true;
  card.dataset.noteId = note.id;
  const preview = (note.content || '').substring(0, 120).replace(/\n/g, ' ');
  const tagsHtml = renderNoteTags(note.tags);
  const dueHtml = isTask ? formatDueDate(note.due_date) : '';
  const tabBadge = note.tab_name ? `<span class="note-tab-badge">${escapeHtml(note.tab_name)}</span>` : '';
  const statusIcon = note.status === 'done' ? '☑' : note.status === 'task' ? '☐' : '';

  card.innerHTML = `
    <div class="note-card-body">
      <div class="note-card-title">${statusIcon ? `<span class="note-status-icon" data-action="toggle-status">${statusIcon}</span> ` : ''}${note.pinned ? '<span class="note-pinned-icon">📌</span> ' : ''}${escapeHtml(note.title || 'Без названия')}</div>
      ${preview ? `<div class="note-card-preview">${escapeHtml(preview)}</div>` : ''}
      <div class="note-card-meta">
        <span>${formatNoteDate(note.updated_at || note.created_at)}</span>
        ${dueHtml ? `<span class="note-card-due">${dueHtml}</span>` : ''}
        ${tabBadge}
      </div>
      ${tagsHtml ? `<div class="note-card-tags">${tagsHtml}</div>` : ''}
    </div>
    <div class="note-card-actions">
      <button class="note-card-action-btn" data-action="pin" title="${note.pinned ? 'Открепить' : 'Закрепить'}">📌</button>
      <button class="note-card-action-btn" data-action="archive" title="${note.archived ? 'Разархивировать' : 'В архив'}">📦</button>
      <button class="note-card-action-btn note-card-action-danger" data-action="delete" title="Удалить">🗑</button>
    </div>`;

  // DnD state — prevent click from firing after drag
  let wasDragged = false;

  // Click card body to open editor
  card.querySelector('.note-card-body').addEventListener('click', (e) => {
    if (wasDragged) { wasDragged = false; return; }
    if (e.target.closest('[data-action="toggle-status"]')) return;
    S.currentNoteId = note.id;
    S.notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, note.id);
  });

  // Toggle task status
  card.querySelector('[data-action="toggle-status"]')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const nextStatus = note.status === 'done' ? 'task' : note.status === 'task' ? 'done' : 'task';
    await invoke('update_note_status', { id: note.id, status: nextStatus }).catch(err => console.error('status:', err));
    if (onRefresh) onRefresh();
  });

  // Pin
  card.querySelector('[data-action="pin"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    await invoke('toggle_note_pin', { id: note.id }).catch(err => console.error('pin:', err));
    if (onRefresh) onRefresh();
  });
  // Archive
  card.querySelector('[data-action="archive"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    await invoke('toggle_note_archive', { id: note.id }).catch(err => console.error('archive:', err));
    if (onRefresh) onRefresh();
  });
  // Delete
  card.querySelector('[data-action="delete"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    if (!(await confirmModal('Удалить заметку?'))) return;
    await invoke('delete_note', { id: note.id }).catch(err => console.error('delete:', err));
    if (onRefresh) onRefresh();
  });

  // DnD events
  card.addEventListener('dragstart', (e) => {
    wasDragged = true;
    card.classList.add('dragging');
    e.dataTransfer.setData('text/plain', String(note.id));
    e.dataTransfer.effectAllowed = 'move';
  });
  card.addEventListener('dragend', () => card.classList.remove('dragging'));

  return card;
}

function setupNotesDnD(list) {
  let dragOverCard = null;
  list.addEventListener('dragover', (e) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const card = e.target.closest('.note-card');
    if (card && card !== dragOverCard) {
      dragOverCard?.classList.remove('drag-over');
      card.classList.add('drag-over');
      dragOverCard = card;
    }
  });
  list.addEventListener('dragleave', (e) => {
    if (!list.contains(e.relatedTarget)) {
      dragOverCard?.classList.remove('drag-over');
      dragOverCard = null;
    }
  });
  list.addEventListener('drop', async (e) => {
    e.preventDefault();
    dragOverCard?.classList.remove('drag-over');
    const draggedId = parseInt(e.dataTransfer.getData('text/plain'));
    const targetCard = e.target.closest('.note-card');
    if (!targetCard) return;
    const targetId = parseInt(targetCard.dataset.noteId);
    if (draggedId === targetId) return;

    // Reorder DOM
    const cards = [...list.querySelectorAll('.note-card')];
    const ids = cards.map(c => parseInt(c.dataset.noteId));
    const fromIdx = ids.indexOf(draggedId);
    const toIdx = ids.indexOf(targetId);
    if (fromIdx < 0 || toIdx < 0) return;
    ids.splice(fromIdx, 1);
    ids.splice(toIdx, 0, draggedId);
    await invoke('reorder_notes', { ids }).catch(err => console.error('reorder:', err));
    loadNotes();
  });
}

async function createAndOpenNote() {
  try {
    const id = await invoke('create_note', { title: '', content: '', tags: '', tabName: null, status: null, dueDate: null, reminderAt: null });
    S.currentNoteId = id;
    S.notesViewMode = 'edit';
    const el = document.getElementById('notes-content');
    if (el) renderNoteEditor(el, id);
  } catch (err) { console.error('create_note error:', err); }
}

async function saveCurrentNote(id) {
  clearTimeout(S.noteAutoSaveTimeout);
  const title = document.getElementById('note-title')?.value || '';
  const tags = document.getElementById('note-tags-input')?.value || '';
  const tabName = document.getElementById('note-tab-select')?.value || null;
  const status = document.getElementById('note-status-select')?.value || null;
  const dueDate = document.getElementById('note-due-date')?.value || null;
  const reminderAt = document.getElementById('note-reminder')?.value || null;
  // Get content from Editor.js if available
  let content = '';
  let contentBlocks = null;
  if (S.currentNoteEditor) {
    try {
      const output = await S.currentNoteEditor.save();
      contentBlocks = JSON.stringify(output);
      content = blocksToPlainText(output);
    } catch (e) { console.error('Editor.js save error:', e); }
  }
  return invoke('update_note', { id, title, content, tags, pinned: null, archived: null, tabName, status, dueDate, reminderAt, contentBlocks });
}

async function renderNoteEditor(el, id) {
  try {
    const note = await invoke('get_note', { id });

    // Build tab options
    const tabKeys = Object.keys(TAB_REGISTRY).filter(k => k !== 'chat' && !k.startsWith('page_'));
    const tabOptions = tabKeys.map(k => `<option value="${k}" ${note.tab_name === k ? 'selected' : ''}>${TAB_REGISTRY[k].label}</option>`).join('');

    // Parse tags for pills
    const tags = (note.tags || '').split(',').map(t => t.trim()).filter(Boolean);
    const tagPillsHtml = tags.map(t => {
      const color = S.tagColorMap[t] || 'blue';
      return `<span class="note-tag-pill badge-${color}" data-tag="${escapeHtml(t)}">${escapeHtml(t)} <span class="note-tag-remove">×</span></span>`;
    }).join('');

    el.innerHTML = `<div class="page-content note-edit-view">
      <div class="note-edit-topbar">
        <div class="note-breadcrumb" id="note-back-btn">← Notes</div>
        <div class="note-edit-actions">
          <button class="note-action-btn ${note.pinned ? 'active' : ''}" id="note-pin-btn" title="${note.pinned ? 'Открепить' : 'Закрепить'}">📌</button>
          <button class="note-action-btn" id="note-archive-btn" title="${note.archived ? 'Разархивировать' : 'В архив'}">📦</button>
          <button class="note-action-btn note-action-btn-danger" id="note-delete-btn" title="Удалить">🗑</button>
        </div>
      </div>

      <div class="note-task-bar">
        <select class="form-select note-status-select" id="note-status-select">
          <option value="note" ${note.status === 'note' ? 'selected' : ''}>Заметка</option>
          <option value="task" ${note.status === 'task' ? 'selected' : ''}>Задача</option>
          <option value="done" ${note.status === 'done' ? 'selected' : ''}>Выполнено</option>
        </select>
        <input type="date" class="form-input note-due-input" id="note-due-date" value="${note.due_date || ''}" placeholder="Дедлайн">
        <input type="datetime-local" class="form-input note-reminder-input" id="note-reminder" value="${note.reminder_at || ''}" placeholder="Напомнить">
        <select class="form-select note-tab-select" id="note-tab-select">
          <option value="">— Без таба —</option>
          ${tabOptions}
        </select>
      </div>

      <input class="page-title-input" id="note-title" value="${escapeHtml(note.title || '')}" placeholder="Без названия">

      <div class="note-tags-row" id="note-tags-row">
        ${tagPillsHtml}
        <input class="note-tag-input" id="note-tag-add" placeholder="+ тег" autocomplete="off">
      </div>

      <div id="note-editor" class="block-editor-container"></div>
      <input type="hidden" id="note-tags-input" value="${escapeHtml(note.tags || '')}">
    </div>`;

    // Initialize Editor.js
    let editorData = null;
    if (note.content_blocks) {
      try { editorData = JSON.parse(note.content_blocks); } catch (e) { console.error('parse content_blocks:', e); }
    }
    if (!editorData && note.content) {
      editorData = migrateTextToBlocks(note.content);
    }

    const autoSave = () => {
      clearTimeout(S.noteAutoSaveTimeout);
      S.noteAutoSaveTimeout = setTimeout(() => {
        saveCurrentNote(id).catch(e => console.error('note autosave error:', e));
      }, 800);
    };

    // Destroy previous editor instance
    if (S.currentNoteEditor) {
      try { S.currentNoteEditor.destroy(); } catch (e) {}
      S.currentNoteEditor = null;
    }
    S.currentNoteEditor = initBlockEditor('note-editor', editorData, () => autoSave());

    if (!note.title) document.getElementById('note-title')?.focus();

    document.getElementById('note-title')?.addEventListener('input', autoSave);
    document.getElementById('note-status-select')?.addEventListener('change', autoSave);
    document.getElementById('note-due-date')?.addEventListener('change', autoSave);
    document.getElementById('note-reminder')?.addEventListener('change', autoSave);
    document.getElementById('note-tab-select')?.addEventListener('change', autoSave);

    // Tag input
    const tagInput = document.getElementById('note-tag-add');
    const tagsRow = document.getElementById('note-tags-row');
    tagInput?.addEventListener('keydown', async (e) => {
      if (e.key === 'Enter' || e.key === ',') {
        e.preventDefault();
        const val = tagInput.value.trim().replace(/,/g, '');
        if (!val) return;
        const hidden = document.getElementById('note-tags-input');
        const curTags = (hidden.value || '').split(',').map(t => t.trim()).filter(Boolean);
        if (!curTags.includes(val)) {
          curTags.push(val);
          hidden.value = curTags.join(', ');
          // Ensure tag has a color
          if (!S.tagColorMap[val]) {
            const colors = ['blue','green','purple','orange','yellow','red','pink'];
            S.tagColorMap[val] = colors[curTags.length % colors.length];
            invoke('set_note_tag_color', { name: val, color: S.tagColorMap[val] }).catch(() => {});
          }
          const pill = document.createElement('span');
          pill.className = `note-tag-pill badge-${S.tagColorMap[val]}`;
          pill.dataset.tag = val;
          pill.innerHTML = `${escapeHtml(val)} <span class="note-tag-remove">×</span>`;
          tagsRow.insertBefore(pill, tagInput);
        }
        tagInput.value = '';
        autoSave();
      }
    });

    // Remove tag on click ×
    tagsRow?.addEventListener('click', (e) => {
      const rm = e.target.closest('.note-tag-remove');
      if (!rm) return;
      const pill = rm.closest('.note-tag-pill');
      const tag = pill?.dataset.tag;
      if (!tag) return;
      pill.remove();
      const hidden = document.getElementById('note-tags-input');
      hidden.value = (hidden.value || '').split(',').map(t => t.trim()).filter(t => t !== tag).join(', ');
      autoSave();
    });

    // Back
    document.getElementById('note-back-btn')?.addEventListener('click', async () => {
      await saveCurrentNote(id).catch(() => {});
      if (S.currentNoteEditor) {
        try { S.currentNoteEditor.destroy(); } catch (e) {}
        S.currentNoteEditor = null;
      }
      S.currentNoteId = null;
      S.notesViewMode = 'list';
      loadNotes();
    });

    // Pin
    document.getElementById('note-pin-btn')?.addEventListener('click', async () => {
      try {
        await saveCurrentNote(id);
        await invoke('toggle_note_pin', { id });
        renderNoteEditor(el, id);
      } catch (err) { console.error('pin error:', err); }
    });

    // Archive
    document.getElementById('note-archive-btn')?.addEventListener('click', async () => {
      try {
        await saveCurrentNote(id);
        await invoke('toggle_note_archive', { id });
        S.currentNoteId = null;
        S.notesViewMode = 'list';
        loadNotes();
      } catch (err) { console.error('archive error:', err); }
    });

    // Delete
    document.getElementById('note-delete-btn')?.addEventListener('click', async () => {
      if (!(await confirmModal('Удалить заметку?'))) return;
      try {
        await invoke('delete_note', { id });
        S.currentNoteId = null;
        S.notesViewMode = 'list';
        loadNotes();
      } catch (err) { console.error('delete error:', err); }
    });

  } catch (e) {
    console.error('renderNoteEditor error:', e);
    el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">📝</div><div class="empty-state-text">Заметка не найдена</div></div>`;
  }
}

async function renderLinkedNotes(container, tabName) {
  try {
    const notes = await invoke('get_notes_for_tab', { tabName });
    if (!notes || notes.length === 0) return;
    const section = document.createElement('div');
    section.className = 'linked-notes-section';
    section.innerHTML = `<div class="linked-notes-header"><span>📝 Связанные заметки</span><span class="linked-notes-count">${notes.length}</span></div>`;
    const list = document.createElement('div');
    list.className = 'linked-notes-list';
    for (const n of notes.slice(0, 10)) {
      const item = document.createElement('div');
      item.className = `linked-note-item${n.status === 'done' ? ' task-done' : ''}`;
      const statusIcon = n.status === 'done' ? '☑ ' : n.status === 'task' ? '☐ ' : '';
      item.innerHTML = `<span class="linked-note-title">${statusIcon}${escapeHtml(n.title || 'Без названия')}</span>
        <span class="linked-note-date">${formatNoteDate(n.updated_at)}</span>`;
      item.addEventListener('click', () => {
        openTab('notes');
        setTimeout(() => {
          S.currentNoteId = n.id;
          S.notesViewMode = 'edit';
          const el = document.getElementById('notes-content');
          if (el) renderNoteEditor(el, n.id);
        }, 100);
      });
      list.appendChild(item);
    }
    section.appendChild(list);
    container.appendChild(section);
  } catch (_) {}
}

// ── Database View System (Notion-style) ──

async function renderDatabaseView(el, tabId, recordTable, records, options = {}) {
  const { fixedColumns = [], idField = 'id', onRowClick, addButton, reloadFn } = options;

  // Load custom property definitions
  let customProps = [];
  try { customProps = await invoke('get_property_definitions', { tabId }); } catch {}

  // Load property values for all records
  const recordIds = records.map(r => r[idField]);
  let allValues = [];
  if (recordIds.length > 0 && customProps.length > 0) {
    try { allValues = await invoke('get_property_values', { recordTable, recordIds }); } catch {}
  }

  // Build values map: { recordId: { propertyId: value } }
  const valuesMap = {};
  for (const v of allValues) {
    if (!valuesMap[v.record_id]) valuesMap[v.record_id] = {};
    valuesMap[v.record_id][v.property_id] = v.value;
  }

  // Load and apply filters
  if (!S.dbvFilters[tabId]) await loadFiltersFromViewConfig(tabId);
  const filteredRecords = applyFilters(records, valuesMap, S.dbvFilters[tabId], idField);

  const visibleProps = customProps.filter(p => p.visible !== false);

  // Render header
  const headerHtml = `<div class="database-view-header">
    ${addButton ? `<button class="btn-primary" id="dbv-add-btn">${addButton}</button>` : ''}
  </div>`;

  // Render table
  const thFixed = fixedColumns.map(c => `<th class="sortable-header" data-sort="${c.key}">${c.label}</th>`).join('');
  const thCustom = visibleProps.map(p =>
    `<th class="sortable-header prop-header" data-sort="prop_${p.id}" data-prop-id="${p.id}"><span class="col-type-icon">${getTypeIcon(p.type)}</span>${escapeHtml(p.name)}</th>`
  ).join('');
  const thAddCol = `<th class="add-prop-col" id="dbv-add-prop-col" title="Добавить свойство">+</th>`;

  let tbodyHtml = '';
  for (const record of filteredRecords) {
    const rid = record[idField];
    const tdFixed = fixedColumns.map(c => {
      const val = c.render ? c.render(record) : escapeHtml(String(record[c.key] ?? ''));
      return `<td>${val}</td>`;
    }).join('');

    const tdCustom = visibleProps.map(p => {
      const rawVal = valuesMap[rid]?.[p.id] ?? '';
      const displayVal = formatPropValue(rawVal, p);
      return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}'>${displayVal}</td>`;
    }).join('');

    tbodyHtml += `<tr class="data-table-row" data-id="${rid}">${tdFixed}${tdCustom}<td></td></tr>`;
  }

  if (filteredRecords.length === 0) {
    const colspan = fixedColumns.length + visibleProps.length + 1;
    tbodyHtml = `<tr><td colspan="${colspan}" style="text-align:center;color:var(--text-faint);padding:24px;">Пока пусто</td></tr>`;
  }

  el.innerHTML = headerHtml + `
    <table class="data-table database-view">
      <thead><tr>${thFixed}${thCustom}${thAddCol}</tr></thead>
      <tbody>${tbodyHtml}</tbody>
    </table>`;

  // Render filter bar if custom props exist
  if (customProps.length > 0) {
    renderFilterBar(el, tabId, customProps, reloadFn || (() => {}));
  }

  // Bind row click
  if (onRowClick) {
    el.querySelectorAll('.data-table-row').forEach(row => {
      row.style.cursor = 'pointer';
      row.addEventListener('click', (e) => {
        if (e.target.closest('.cell-editable') && e.target.closest('.inline-editor')) return;
        const id = parseInt(row.dataset.id);
        const record = filteredRecords.find(r => r[idField] === id);
        if (record) onRowClick(record);
      });
    });
  }

  // Bind inline editing
  el.querySelectorAll('.cell-editable').forEach(cell => {
    cell.addEventListener('click', (e) => {
      e.stopPropagation();
      startInlineEdit(cell, recordTable, reloadFn);
    });
  });

  // Bind + column header to add property
  el.querySelector('#dbv-add-prop-col')?.addEventListener('click', () => {
    showAddPropertyModal(tabId, reloadFn);
  });

  // Bind custom property header clicks to context menu
  el.querySelectorAll('.prop-header').forEach(th => {
    th.addEventListener('click', (e) => {
      e.stopPropagation();
      const propId = parseInt(th.dataset.propId);
      const prop = customProps.find(p => p.id === propId);
      if (!prop) return;
      const rect = th.getBoundingClientRect();
      showColumnMenu(prop, rect, tabId, recordTable, reloadFn, el, records, allValues, fixedColumns, visibleProps, valuesMap, idField, options);
    });
  });

  // Bind add button
  if (addButton && options.onAdd) {
    document.getElementById('dbv-add-btn')?.addEventListener('click', options.onAdd);
  }

  // Bind sortable headers (fixed columns only — custom props use context menu)
  el.querySelectorAll('.sortable-header:not(.prop-header)').forEach(th => {
    th.addEventListener('click', () => {
      const sortKey = th.dataset.sort;
      const currentDir = th.dataset.dir || 'none';
      const newDir = currentDir === 'asc' ? 'desc' : 'asc';
      el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
      th.dataset.dir = newDir;
      th.classList.add(`sort-${newDir}`);
      sortDatabaseView(el, records, allValues, sortKey, newDir, fixedColumns, visibleProps, valuesMap, idField, options);
    });
  });
}

function formatPropValue(val, prop) {
  if (!val && val !== 0) return '<span class="text-faint">—</span>';
  switch (prop.type) {
    case 'checkbox': return val === 'true' ? '✓' : '—';
    case 'select': return `<span class="badge badge-blue">${escapeHtml(val)}</span>`;
    case 'multi_select': {
      try {
        const items = JSON.parse(val);
        return items.map(i => `<span class="badge badge-purple">${escapeHtml(i)}</span>`).join(' ');
      } catch { return escapeHtml(val); }
    }
    case 'url': return `<a href="${escapeHtml(val)}" target="_blank" style="color:var(--accent-blue);text-decoration:none;">${escapeHtml(val.substring(0, 30))}</a>`;
    case 'number': return escapeHtml(val);
    case 'date': return escapeHtml(val);
    default: return escapeHtml(val);
  }
}

function startInlineEdit(cell, recordTable, reloadFn) {
  if (cell.querySelector('.inline-editor')) return;
  const recordId = parseInt(cell.dataset.recordId);
  const propId = parseInt(cell.dataset.propId);
  const propType = cell.dataset.propType;
  let options = [];
  try { options = JSON.parse(cell.dataset.propOptions || '[]'); } catch {}

  const currentVal = cell.textContent.trim();
  const originalHtml = cell.innerHTML;

  let editorHtml = '';
  switch (propType) {
    case 'select':
      editorHtml = `<select class="inline-editor inline-select">
        <option value="">—</option>
        ${options.map(o => `<option value="${escapeHtml(o)}"${o === currentVal ? ' selected' : ''}>${escapeHtml(o)}</option>`).join('')}
      </select>`;
      break;
    case 'multi_select': {
      let selected = [];
      try { selected = JSON.parse(cell.dataset.currentValue || '[]'); } catch {}
      editorHtml = `<div class="inline-editor inline-multi-select">
        ${options.map(o => `<label class="inline-ms-option"><input type="checkbox" value="${escapeHtml(o)}"${selected.includes(o) ? ' checked' : ''}> ${escapeHtml(o)}</label>`).join('')}
        <button class="btn-primary inline-ms-done" style="font-size:11px;padding:2px 8px;margin-top:4px;">OK</button>
      </div>`;
      break;
    }
    case 'checkbox': {
      // Toggle immediately
      const newVal = currentVal === '✓' ? 'false' : 'true';
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: newVal })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
      return;
    }
    case 'date':
      editorHtml = `<input type="date" class="inline-editor inline-date" value="${currentVal === '—' ? '' : currentVal}">`;
      break;
    case 'number':
      editorHtml = `<input type="number" class="inline-editor inline-number" value="${currentVal === '—' ? '' : currentVal}">`;
      break;
    default:
      editorHtml = `<input type="text" class="inline-editor inline-text" value="${currentVal === '—' ? '' : escapeHtml(currentVal)}">`;
  }

  cell.innerHTML = editorHtml;
  const editor = cell.querySelector('.inline-editor');
  if (editor.tagName === 'INPUT' || editor.tagName === 'SELECT') {
    editor.focus();
    const saveAndClose = () => {
      const val = editor.value || null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    };
    editor.addEventListener('blur', saveAndClose);
    editor.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); editor.blur(); }
      if (e.key === 'Escape') { cell.innerHTML = originalHtml; }
    });
  } else if (propType === 'multi_select') {
    cell.querySelector('.inline-ms-done')?.addEventListener('click', () => {
      const checked = [...cell.querySelectorAll('input:checked')].map(cb => cb.value);
      const val = checked.length > 0 ? JSON.stringify(checked) : null;
      cell.innerHTML = originalHtml;
      invoke('set_property_value', { recordId, recordTable, propertyId: propId, value: val })
        .then(() => { if (reloadFn) reloadFn(); }).catch(() => {});
    });
  }
}

// ── Property System ──

function showAddPropertyModal(tabId, reloadFn) {
  let selectedType = 'text';
  let optionsList = [];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';

  function renderModal() {
    const needsOptions = ['select', 'multi_select'].includes(selectedType);
    const typeGrid = PROPERTY_TYPE_DEFS.map(t =>
      `<div class="prop-type-card${t.id === selectedType ? ' selected' : ''}" data-type="${t.id}">
        <div class="prop-type-icon">${t.icon}</div>
        <div class="prop-type-name">${t.name}</div>
      </div>`
    ).join('');

    const optionsHtml = needsOptions ? `
      <div class="prop-config-section">
        <div class="prop-section-label">Варианты</div>
        <div class="prop-options-container">
          <div class="prop-options-tags" id="prop-tags">
            ${optionsList.map((o, i) => `<span class="prop-option-tag">${escapeHtml(o)}<span class="prop-option-tag-remove" data-idx="${i}">&times;</span></span>`).join('')}
          </div>
          <div class="prop-option-add">
            <input id="prop-option-input" type="text" placeholder="Новый вариант..." autocomplete="off">
            <button id="prop-option-add-btn">+</button>
          </div>
        </div>
      </div>` : '';

    overlay.innerHTML = `<div class="modal modal-property">
      <div class="modal-title">Новое свойство</div>
      <div class="form-group"><label class="form-label">Название</label><input class="form-input" id="prop-name" placeholder="Без названия" autocomplete="off"></div>
      <div class="form-group">
        <label class="form-label">Тип</label>
        <div class="prop-type-grid">${typeGrid}</div>
      </div>
      ${optionsHtml}
      <div class="modal-actions">
        <button class="btn-secondary" id="prop-cancel">Отмена</button>
        <button class="btn-primary" id="prop-save">Добавить</button>
      </div>
    </div>`;

    // Bind type selection
    overlay.querySelectorAll('.prop-type-card').forEach(card => {
      card.addEventListener('click', () => {
        selectedType = card.dataset.type;
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        const nameInput = document.getElementById('prop-name');
        if (nameInput) nameInput.value = nameVal;
      });
    });

    // Bind option tag removal
    overlay.querySelectorAll('.prop-option-tag-remove').forEach(btn => {
      btn.addEventListener('click', () => {
        const idx = parseInt(btn.dataset.idx);
        optionsList.splice(idx, 1);
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        document.getElementById('prop-name').value = nameVal;
      });
    });

    // Bind add option
    const addOptBtn = overlay.querySelector('#prop-option-add-btn');
    const addOptInput = overlay.querySelector('#prop-option-input');
    const addOption = () => {
      const val = addOptInput?.value?.trim();
      if (val && !optionsList.includes(val)) {
        optionsList.push(val);
        const nameVal = document.getElementById('prop-name')?.value || '';
        renderModal();
        document.getElementById('prop-name').value = nameVal;
        document.getElementById('prop-option-input')?.focus();
      }
    };
    addOptBtn?.addEventListener('click', addOption);
    addOptInput?.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addOption(); } });

    // Bind cancel
    overlay.querySelector('#prop-cancel')?.addEventListener('click', () => overlay.remove());

    // Bind save
    overlay.querySelector('#prop-save')?.addEventListener('click', async () => {
      const name = document.getElementById('prop-name')?.value?.trim() || 'Без названия';
      let options = null;
      if (['select', 'multi_select'].includes(selectedType) && optionsList.length > 0) {
        options = JSON.stringify(optionsList);
      }
      try {
        await invoke('create_property_definition', { tabId, name, propType: selectedType, position: null, color: null, options, defaultValue: null });
        overlay.remove();
        if (reloadFn) reloadFn();
      } catch (err) { alert('Error: ' + err); }
    });
  }

  renderModal();
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  setTimeout(() => document.getElementById('prop-name')?.focus(), 50);
}

function showColumnMenu(propDef, anchorRect, tabId, recordTable, reloadFn, el, records, allValues, fixedColumns, visibleProps, valuesMap, idField, options) {
  // Remove any existing menu
  document.querySelectorAll('.col-context-menu').forEach(m => m.remove());

  const menu = document.createElement('div');
  menu.className = 'col-context-menu';

  const needsOptions = ['select', 'multi_select'].includes(propDef.type);

  menu.innerHTML = `
    <div class="col-menu-section">
      <input class="col-menu-name-input" value="${escapeHtml(propDef.name)}" id="col-rename-input" autocomplete="off">
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="type">
        <span class="col-menu-icon">${getTypeIcon(propDef.type)}</span>
        <span>${getTypeName(propDef.type)}</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="sort-asc">
        <span class="col-menu-icon">↑</span>
        <span>Сортировка А→Я</span>
      </div>
      <div class="col-menu-item" data-action="sort-desc">
        <span class="col-menu-icon">↓</span>
        <span>Сортировка Я→А</span>
      </div>
    </div>
    <div class="col-menu-section">
      <div class="col-menu-item" data-action="hide">
        <span class="col-menu-icon">◻</span>
        <span>Скрыть</span>
      </div>
      <div class="col-menu-item col-menu-item danger" data-action="delete">
        <span class="col-menu-icon">✕</span>
        <span>Удалить</span>
      </div>
    </div>
  `;

  // Position menu below the header
  menu.style.left = Math.min(anchorRect.left, window.innerWidth - 220) + 'px';
  menu.style.top = anchorRect.bottom + 4 + 'px';

  document.body.appendChild(menu);

  // Rename on Enter/blur
  const renameInput = menu.querySelector('#col-rename-input');
  const doRename = async () => {
    const newName = renameInput.value.trim();
    if (newName && newName !== propDef.name) {
      try {
        await invoke('update_property_definition', { id: propDef.id, name: newName, propType: null, position: null, color: null, options: null, visible: null });
        if (reloadFn) reloadFn();
      } catch {}
    }
  };
  renameInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); doRename(); menu.remove(); }
    if (e.key === 'Escape') { menu.remove(); }
    e.stopPropagation();
  });
  renameInput.addEventListener('blur', () => { doRename(); });

  // Menu item clicks
  menu.querySelectorAll('.col-menu-item').forEach(item => {
    item.addEventListener('click', async () => {
      const action = item.dataset.action;
      switch (action) {
        case 'sort-asc':
        case 'sort-desc': {
          const dir = action === 'sort-asc' ? 'asc' : 'desc';
          const sortKey = `prop_${propDef.id}`;
          el.querySelectorAll('.sortable-header').forEach(h => { h.dataset.dir = 'none'; h.classList.remove('sort-asc', 'sort-desc'); });
          sortDatabaseView(el, records, allValues, sortKey, dir, fixedColumns, visibleProps, valuesMap, idField, options);
          menu.remove();
          break;
        }
        case 'hide':
          try {
            await invoke('update_property_definition', { id: propDef.id, name: null, propType: null, position: null, color: null, options: null, visible: false });
            if (reloadFn) reloadFn();
          } catch {}
          menu.remove();
          break;
        case 'delete':
          if (await confirmModal(`Удалить свойство "${propDef.name}"?`)) {
            try {
              await invoke('delete_property_definition', { id: propDef.id });
              if (reloadFn) reloadFn();
            } catch {}
          }
          menu.remove();
          break;
      }
    });
  });

  // Close on outside click
  const closeMenu = (e) => {
    if (!menu.contains(e.target)) {
      doRename();
      menu.remove();
      document.removeEventListener('mousedown', closeMenu);
    }
  };
  setTimeout(() => document.addEventListener('mousedown', closeMenu), 10);
}

function sortDatabaseView(el, records, allValues, sortKey, dir, fixedColumns, visibleProps, valuesMap, idField, options) {
  const sorted = [...records].sort((a, b) => {
    let va, vb;
    if (sortKey.startsWith('prop_')) {
      const pid = parseInt(sortKey.substring(5));
      va = valuesMap[a[idField]]?.[pid] ?? '';
      vb = valuesMap[b[idField]]?.[pid] ?? '';
    } else {
      va = a[sortKey] ?? '';
      vb = b[sortKey] ?? '';
    }
    if (typeof va === 'number' && typeof vb === 'number') return dir === 'asc' ? va - vb : vb - va;
    return dir === 'asc' ? String(va).localeCompare(String(vb)) : String(vb).localeCompare(String(va));
  });
  renderDatabaseView(el, options._tabId || '', options._recordTable || '', sorted, options);
}

// ── Filter System ──

function renderFilterBar(el, tabId, customProps, onApply) {
  const filters = S.dbvFilters[tabId] || [];
  const chips = filters.map((f, idx) => {
    const prop = customProps.find(p => p.id === f.propId);
    const label = prop ? prop.name : '?';
    const condLabels = { eq: '=', neq: '\u2260', contains: '\u2248', empty: 'empty', not_empty: 'not empty' };
    return `<span class="filter-chip" data-idx="${idx}">
      ${escapeHtml(label)} ${condLabels[f.condition] || f.condition} ${f.value ? escapeHtml(f.value) : ''}
      <span class="filter-chip-remove" data-remove="${idx}">\u00d7</span>
    </span>`;
  }).join('');

  const bar = document.createElement('div');
  bar.className = 'filter-bar';
  bar.innerHTML = `<button class="btn-secondary" id="dbv-add-filter" style="font-size:11px;padding:4px 10px;">+ Filter</button>${chips}`;
  el.prepend(bar);

  bar.querySelector('#dbv-add-filter')?.addEventListener('click', () => {
    showFilterBuilderModal(tabId, customProps, onApply);
  });

  bar.querySelectorAll('.filter-chip-remove').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const idx = parseInt(btn.dataset.remove);
      if (S.dbvFilters[tabId]) S.dbvFilters[tabId].splice(idx, 1);
      saveFiltersToViewConfig(tabId);
      onApply();
    });
  });
}

function showFilterBuilderModal(tabId, customProps, onApply) {
  if (customProps.length === 0) { alert('Add custom properties first to filter by them.'); return; }
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add Filter</div>
    <div class="form-group"><label class="form-label">Property</label>
      <select class="form-select" id="filter-prop" style="width:100%;">
        ${customProps.map(p => `<option value="${p.id}" data-type="${p.type}" data-options='${escapeHtml(p.options||"[]")}'>${escapeHtml(p.name)}</option>`).join('')}
      </select>
    </div>
    <div class="form-group"><label class="form-label">Condition</label>
      <select class="form-select" id="filter-cond" style="width:100%;">
        <option value="eq">Equals</option><option value="neq">Not equals</option>
        <option value="contains">Contains</option>
        <option value="empty">Is empty</option><option value="not_empty">Is not empty</option>
      </select>
    </div>
    <div class="form-group" id="filter-val-group"><label class="form-label">Value</label>
      <input class="form-input" id="filter-val" placeholder="Value">
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="filter-apply">Apply</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });

  // Update value input based on property type
  const updateValueInput = () => {
    const sel = document.getElementById('filter-prop');
    const opt = sel?.selectedOptions[0];
    const type = opt?.dataset.type;
    const cond = document.getElementById('filter-cond')?.value;
    const valGroup = document.getElementById('filter-val-group');

    if (cond === 'empty' || cond === 'not_empty') {
      valGroup.style.display = 'none';
      return;
    }
    valGroup.style.display = 'block';

    if (type === 'select' || type === 'multi_select') {
      let options = [];
      try { options = JSON.parse(opt?.dataset.options || '[]'); } catch {}
      valGroup.innerHTML = `<label class="form-label">Value</label>
        <select class="form-select" id="filter-val" style="width:100%;">
          ${options.map(o => `<option value="${escapeHtml(o)}">${escapeHtml(o)}</option>`).join('')}
        </select>`;
    } else {
      valGroup.innerHTML = `<label class="form-label">Value</label><input class="form-input" id="filter-val" placeholder="Value">`;
    }
  };

  document.getElementById('filter-prop')?.addEventListener('change', updateValueInput);
  document.getElementById('filter-cond')?.addEventListener('change', updateValueInput);

  document.getElementById('filter-apply')?.addEventListener('click', () => {
    const propId = parseInt(document.getElementById('filter-prop')?.value);
    const condition = document.getElementById('filter-cond')?.value || 'eq';
    const value = document.getElementById('filter-val')?.value || '';
    if (!S.dbvFilters[tabId]) S.dbvFilters[tabId] = [];
    S.dbvFilters[tabId].push({ propId, condition, value });
    overlay.remove();
    saveFiltersToViewConfig(tabId);
    onApply();
  });
}

function applyFilters(records, valuesMap, filters, idField) {
  if (!filters || filters.length === 0) return records;
  return records.filter(r => {
    const rid = r[idField];
    return filters.every(f => {
      const val = valuesMap[rid]?.[f.propId] ?? '';
      switch (f.condition) {
        case 'eq': return val === f.value;
        case 'neq': return val !== f.value;
        case 'contains': return String(val).toLowerCase().includes(f.value.toLowerCase());
        case 'empty': return !val;
        case 'not_empty': return !!val;
        default: return true;
      }
    });
  });
}

async function saveFiltersToViewConfig(tabId) {
  const filters = S.dbvFilters[tabId] || [];
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0) {
      await invoke('update_view_config', { id: configs[0].id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    } else {
      const id = await invoke('create_view_config', { tabId, name: 'Default', viewType: 'table' });
      await invoke('update_view_config', { id, filterJson: JSON.stringify(filters), sortJson: null, visibleColumns: null });
    }
  } catch {}
}

async function loadFiltersFromViewConfig(tabId) {
  try {
    const configs = await invoke('get_view_configs', { tabId });
    if (configs.length > 0 && configs[0].filter_json) {
      S.dbvFilters[tabId] = JSON.parse(configs[0].filter_json);
    }
  } catch {}
}

// ── Register tab loader ──
tabLoaders.loadNotes = loadNotes;

// ── Exports ──
export {
  loadNotes,
  renderDatabaseView,
  renderNoteEditor,
  renderLinkedNotes,
  createAndOpenNote,
  createAndOpenTask,
  formatNoteDate,
};
