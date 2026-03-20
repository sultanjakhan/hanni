// ── tab-data-hobbies.js — Hobbies tab (media collections) ──

import { S, invoke, MEDIA_TYPES, MEDIA_LABELS, STATUS_LABELS } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Hobbies (Media Collections) ──
export async function loadHobbies(subTab) {
  const el = document.getElementById('hobbies-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'hobbies', {
    title: 'Hobbies',
    subtitle: 'Медиа-коллекции',
    icon: '🎮',
    renderDash: async (paneEl) => {
      await loadHobbiesOverview(paneEl);
    },
    renderTable: async (paneEl) => {
      paneEl.innerHTML = '<div id="hobbies-inner-content"></div>';
      const innerEl = paneEl.querySelector('#hobbies-inner-content');
      await loadMediaList(innerEl, null);
    },
  });
}

async function loadHobbiesOverview(el) {
  try {
    const stats = await invoke('get_media_stats', { mediaType: null }).catch(() => ({}));
    const lists = await invoke('get_user_lists').catch(() => []);
    const colors = ['blue', 'green', 'yellow', 'purple', 'red', 'blue', 'green', 'yellow', 'purple'];
    el.innerHTML = `
      <div class="uni-dash-grid">
        ${MEDIA_TYPES.map((t, i) => `<div class="uni-dash-card ${colors[i % colors.length]}"><div class="uni-dash-value">${stats[t] || 0}</div><div class="uni-dash-label">${MEDIA_LABELS[t]}</div></div>`).join('')}
      </div>
      ${lists.length > 0 ? `<div class="uni-dash-grid" style="margin-top:var(--space-3);">
        ${lists.map(l => `<div class="uni-dash-card gray" style="cursor:pointer;" data-list="${l.id}">
          <div class="uni-dash-value">${l.item_count || 0}</div>
          <div class="uni-dash-label">${escapeHtml(l.name)}</div>
        </div>`).join('')}</div>` : ''}
      <div style="margin-top:var(--space-3);">
        <button class="btn-primary" id="create-list-btn">+ New List</button>
      </div>`;
    document.getElementById('create-list-btn')?.addEventListener('click', () => {
      const name = prompt('List name:');
      if (name) invoke('create_user_list', { name, description: '', color: '#9B9B9B' }).then(() => loadHobbies('Overview')).catch(e => alert(e));
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadMediaList(el, mediaType) {
  try {
    const items = await invoke('get_media_items', { mediaType, status: null, hidden: false });

    const fixedColumns = [
      { key: 'title', label: 'Название', editType: 'text', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
      { key: 'media_type', label: 'Тип', editType: 'select', editOptions: ['music','anime','manga','movies','series','cartoons','games','books','podcasts'], render: r => `<span class="badge badge-gray">${MEDIA_LABELS[r.media_type] || r.media_type || ''}</span>` },
      { key: 'status', label: 'Статус', editType: 'select', editOptions: ['planned','in_progress','completed','on_hold','dropped'], render: r => `<span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : r.status === 'dropped' ? 'badge-red' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>` },
      { key: 'rating', label: 'Оценка', editType: 'text', render: r => {
        const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '\u2014';
        return `<span style="color:var(--text-secondary);font-size:12px;">${stars}</span>`;
      }},
      { key: 'year', label: 'Год', editType: 'text', render: r => `<span style="color:var(--text-muted);font-size:12px;">${r.year || '\u2014'}</span>` },
    ];

    el.innerHTML = '<div id="media-dbv"></div>';
    const dbvEl = document.getElementById('media-dbv');

    const dbv = new DatabaseView(dbvEl, {
      tabId: 'hobbies',
      recordTable: 'media_items',
      records: items,
      fixedColumns,
      idField: 'id',
      availableViews: ['table', 'kanban', 'gallery'],
      defaultView: 'table',
      onQuickAdd: async () => {
        await invoke('add_media_item', { mediaType: null, title: null, originalTitle: null, year: null, description: null, coverUrl: null, status: null, rating: null, progress: null, totalEpisodes: null, notes: null });
        loadMediaList(el, mediaType);
      },
      onCellEdit: async (recordId, key, value) => {
        const params = { id: recordId, status: null, rating: null, progress: null, notes: null, title: null, mediaType: null, year: null };
        if (key === 'title') params.title = value;
        else if (key === 'media_type') params.mediaType = value;
        else if (key === 'status') params.status = value;
        else if (key === 'rating') params.rating = value ? parseInt(value) : null;
        else if (key === 'year') params.year = value ? parseInt(value) : null;
        await invoke('update_media_item', params);
        loadMediaList(el, mediaType);
      },
      reloadFn: () => loadMediaList(el, mediaType),
      kanban: {
        groupByField: 'status',
        columns: [
          { key: 'planned', label: 'Planned', icon: '\ud83d\udccb' },
          { key: 'in_progress', label: 'In Progress', icon: '\u25b6' },
          { key: 'completed', label: 'Completed', icon: '\u2705' },
          { key: 'on_hold', label: 'On Hold', icon: '\u23f8' },
          { key: 'dropped', label: 'Dropped', icon: '\u274c' },
        ],
      },
      gallery: {
        minCardWidth: 200,
        renderCard: (r) => {
          const stars = r.rating ? '\u2605'.repeat(Math.round(r.rating / 2)) + '\u2606'.repeat(5 - Math.round(r.rating / 2)) : '';
          return `<div class="dbv-gallery-card-title">${escapeHtml(r.title)}</div>
            <div class="dbv-gallery-card-badges">
              <span class="badge ${r.status === 'completed' ? 'badge-green' : r.status === 'in_progress' ? 'badge-blue' : 'badge-gray'}">${STATUS_LABELS[r.status] || r.status}</span>
              ${r.year ? `<span style="font-size:11px;color:var(--text-muted);">${r.year}</span>` : ''}
            </div>
            ${stars ? `<div style="font-size:12px;color:var(--text-secondary);margin-top:auto;">${stars}</div>` : ''}`;
        },
      },
      onDrop: async (recordId, field, newValue) => {
        try {
          await invoke('update_media_item', {
            id: parseInt(recordId), status: newValue,
            rating: null, progress: null, notes: null, title: null, mediaType: null, year: null,
          });
          loadMediaList(el, mediaType);
        } catch (err) { console.error('kanban drop:', err); }
      },
    });
    S.dbViews[`hobbies_${mediaType || 'all'}`] = dbv;
    await dbv.render();
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

function showAddMediaModal(mediaType) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Add ${MEDIA_LABELS[mediaType]}</div>
    <div class="form-row"><input class="form-input" id="media-title" placeholder="Title"></div>
    <div class="form-row">
      <select class="form-select" id="media-status">
        <option value="planned">Planned</option><option value="in_progress">In Progress</option>
        <option value="completed">Completed</option><option value="on_hold">On Hold</option>
      </select>
      <input class="form-input" id="media-year" type="number" placeholder="Year" style="max-width:80px;">
      <input class="form-input" id="media-rating" type="number" min="0" max="10" placeholder="Rating" style="max-width:80px;">
    </div>
    ${hasEpisodes ? `<div class="form-row">
      <input class="form-input" id="media-progress" type="number" min="0" placeholder="Episode" style="max-width:80px;">
      <span class="form-hint">/</span>
      <input class="form-input" id="media-total" type="number" min="0" placeholder="Total" style="max-width:80px;">
    </div>` : ''}
    <textarea class="form-textarea" id="media-notes" placeholder="Notes" rows="2"></textarea>
    <div class="modal-actions">
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="media-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('media-save')?.addEventListener('click', async () => {
    const title = document.getElementById('media-title')?.value?.trim();
    if (!title) return;
    try {
      await invoke('add_media_item', {
        mediaType, title,
        originalTitle: null, year: parseInt(document.getElementById('media-year')?.value) || null,
        description: null, coverUrl: null,
        status: document.getElementById('media-status')?.value || 'planned',
        rating: parseInt(document.getElementById('media-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('media-progress')?.value) || null) : null,
        totalEpisodes: hasEpisodes ? (parseInt(document.getElementById('media-total')?.value) || null) : null,
        notes: document.getElementById('media-notes')?.value || null,
      });
      overlay.remove();
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
}

function showMediaDetail(item, mediaType) {
  const hasEpisodes = ['anime','series','cartoon','manga','podcast'].includes(mediaType);
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">${escapeHtml(item.title)}</div>
    ${item.year ? `<div style="color:var(--text-secondary);font-size:12px;margin-bottom:8px;">${item.year}</div>` : ''}
    <div class="form-group"><label class="form-label">Status</label>
      <select class="form-select" id="md-status" style="width:100%;">
        ${Object.entries(STATUS_LABELS).map(([k,v]) => `<option value="${k}"${item.status===k?' selected':''}>${v}</option>`).join('')}
      </select></div>
    <div class="form-group"><label class="form-label">Rating (0-10)</label><input class="form-input" id="md-rating" type="number" min="0" max="10" value="${item.rating||''}"></div>
    ${hasEpisodes ? `<div class="form-group"><label class="form-label">Progress</label><input class="form-input" id="md-progress" type="number" value="${item.progress||0}"></div>` : ''}
    <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="md-notes">${escapeHtml(item.notes||'')}</textarea></div>
    <div class="modal-actions">
      <button class="btn-danger" id="md-delete">Delete</button>
      <button class="btn-secondary" onclick="this.closest('.modal-overlay').remove()">Cancel</button>
      <button class="btn-primary" id="md-save">Save</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  document.getElementById('md-save')?.addEventListener('click', async () => {
    try {
      await invoke('update_media_item', {
        id: item.id,
        status: document.getElementById('md-status')?.value || null,
        rating: parseInt(document.getElementById('md-rating')?.value) || null,
        progress: hasEpisodes ? (parseInt(document.getElementById('md-progress')?.value) || null) : null,
        notes: document.getElementById('md-notes')?.value || null,
        title: null, mediaType: null, year: null,
      });
      overlay.remove();
      loadHobbies(MEDIA_LABELS[mediaType]);
    } catch (err) { alert('Error: ' + err); }
  });
  document.getElementById('md-delete')?.addEventListener('click', async () => {
    if (!(await confirmModal('Удалить?'))) return;
    await invoke('delete_media_item', { id: item.id }).catch(e => alert(e));
    overlay.remove();
    loadHobbies(MEDIA_LABELS[mediaType]);
  });
}
