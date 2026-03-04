// ── js/tab-notes.js — Notes tab (view system extracted to db-view/) ──

import { S, invoke, tabLoaders, TAB_REGISTRY } from './state.js';
import { escapeHtml, renderPageHeader, setupPageHeaderControls, confirmModal, skeletonPage, initBlockEditor, blocksToPlainText, migrateTextToBlocks } from './utils.js';
import { openTab } from './tabs.js';
import { DatabaseView } from './db-view/db-view.js';

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

// ── Database View — backward-compat wrapper (logic extracted to db-view/) ──

async function renderDatabaseView(el, tabId, recordTable, records, options = {}) {
  const { fixedColumns = [], idField = 'id', onRowClick, addButton, reloadFn } = options;
  const dbv = new DatabaseView(el, {
    tabId: options._tabId || tabId,
    recordTable: options._recordTable || recordTable,
    records,
    fixedColumns,
    idField,
    availableViews: ['table'],
    defaultView: 'table',
    addButton,
    onAdd: options.onAdd,
    onRowClick,
    reloadFn: reloadFn || (() => {}),
  });
  await dbv.render();
}

// ── Register tab loader ──
tabLoaders.loadNotes = loadNotes;
tabLoaders.renderDatabaseView = renderDatabaseView;

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
