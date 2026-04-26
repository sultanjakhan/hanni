// tab-timeline.js — Timeline tab loader
import { S, invoke, tabLoaders } from './state.js';
import { escapeHtml } from './utils.js';

async function loadTimeline(subTab) {
  const el = document.getElementById('timeline-content');
  if (!el) return;

  // Auto-sync AFK + Focus blocks from activity_snapshots
  const today = localDate();
  await Promise.all([
    invoke('sync_timeline_auto', { date: today }).catch(() => {}),
    invoke('sync_health_to_timeline', { date: today }).catch(() => {}),
  ]);

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'timeline', {
    title: 'Timeline',
    subtitle: '24-часовой обзор активности',
    icon: '⏱️',
    defaultPane: 'today',

    renderToday: async (paneEl) => {
      const { renderTimelineToday } = await import('./timeline-today.js');
      await renderTimelineToday(paneEl);
    },

    renderDash: async (paneEl) => {
      const { renderTimelineDash } = await import('./timeline-dash.js');
      await renderTimelineDash(paneEl);
    },

    renderTracking: async (paneEl) => {
      const { renderTimelineGrid } = await import('./timeline-grid.js');
      await renderTimelineGrid(paneEl);
    },

    renderTable: async (paneEl) => {
      await renderBlocksTable(paneEl);
    },
  }, subTab);
}

async function renderBlocksTable(paneEl) {
  const today = localDate();
  const blocks = await invoke('get_timeline_blocks', { date: today }).catch(() => []);

  const fmtDate = (d) => new Date(d + 'T12:00:00').toLocaleDateString('ru', { day: 'numeric', month: 'short' });

  const rows = blocks.map(b => `
    <tr class="data-table-row" data-id="${b.id}">
      <td class="col-check"><span style="color:${b.type_color}">${b.type_icon}</span></td>
      <td>${escapeHtml(b.type_name)}</td>
      <td>${fmtDate(b.date)}</td>
      <td>${b.start_time} — ${b.end_time}</td>
      <td>${b.duration_minutes} мин</td>
      <td>${escapeHtml(b.source)}</td>
      <td>${escapeHtml(b.notes || '')}</td>
    </tr>`).join('');

  const addRow = `<tr class="dbv-add-row"><td colspan="7"><div class="dbv-add-row-label"><span class="dbv-add-row-plus">+</span></div></td></tr>`;

  paneEl.innerHTML = `
    <div class="dbv-table-wrap">
      <table class="data-table database-view">
        <thead><tr>
          <th class="col-check-header"></th>
          <th>Тип</th><th>Дата</th><th>Время</th><th>Длит.</th><th>Источник</th><th>Заметки</th>
        </tr></thead>
        <tbody>${rows.length ? rows : ''}${addRow}</tbody>
      </table>
      <div class="dbv-table-footer"><span>Записей: ${blocks.length}</span></div>
    </div>`;

  paneEl.querySelector('.dbv-add-row')?.addEventListener('click', async () => {
    const { showBlockModal } = await import('./timeline-blocks.js');
    await showBlockModal(today, null);
    await renderBlocksTable(paneEl);
  });
  paneEl.querySelectorAll('.data-table-row').forEach(row => {
    row.addEventListener('click', async () => {
      const { showBlockModal } = await import('./timeline-blocks.js');
      await showBlockModal(today, null, parseInt(row.dataset.id));
      await renderBlocksTable(paneEl);
    });
  });
}

function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}

// Register tab loader
tabLoaders.loadTimeline = loadTimeline;

export { loadTimeline };
