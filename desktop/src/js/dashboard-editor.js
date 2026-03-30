// dashboard-editor.js — Inline dashboard editor with drag-and-drop, add/edit/delete widgets
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { renderDashboard } from './dashboard-builder.js';

const COLORS = [
  { name: 'blue', hex: '#3b82f6' }, { name: 'green', hex: '#22c55e' }, { name: 'yellow', hex: '#eab308' },
  { name: 'purple', hex: '#8b5cf6' }, { name: 'red', hex: '#ef4444' }, { name: 'orange', hex: '#f97316' },
];
const WIDGET_TYPES = [
  { value: 'stat', label: 'Число' },
  { value: 'interactive', label: 'Интерактивный' },
  { value: 'progress', label: 'Прогресс' },
  { value: 'list', label: 'Список' },
  { value: 'text', label: 'Текст' },
];

export async function enterEditMode(paneEl, tabId) {
  const widgets = await invoke('get_dashboard_widgets', { tabId }).catch(() => []);

  paneEl.innerHTML = '';
  // Toolbar
  const toolbar = document.createElement('div');
  toolbar.className = 'dash-edit-toolbar';
  toolbar.innerHTML = `
    <button class="btn btn-sm btn-primary dash-add-btn">+ Добавить</button>
    <div style="flex:1"></div>
    <button class="btn btn-sm btn-accent dash-done-btn">Готово</button>`;
  paneEl.appendChild(toolbar);

  // Grid
  const grid = document.createElement('div');
  grid.className = 'uni-dash-grid dash-edit-mode';
  paneEl.appendChild(grid);

  let items = widgets.map(w => ({ ...w }));

  function renderEditCards() {
    grid.innerHTML = '';
    items.forEach((w, idx) => {
      const card = document.createElement('div');
      card.className = `uni-dash-card dash-color-${w.config.color || 'blue'} dash-edit-card`;
      card.draggable = true;
      card.dataset.idx = idx;
      card.innerHTML = `
        <div class="dash-card-header">
          <span class="dash-drag-handle">⠿</span>
          <span class="dash-widget-type">${escapeHtml(w.widget_type)}</span>
          <button class="dash-card-edit" title="Изменить"><svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M11.5 1.5l3 3L5 14H2v-3L11.5 1.5z"/></svg></button>
          <button class="dash-card-delete" title="Удалить"><svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><line x1="4" y1="4" x2="12" y2="12"/><line x1="12" y1="4" x2="4" y2="12"/></svg></button>
        </div>
        <div class="uni-dash-value">${escapeHtml(w.config.label || '?')}</div>
        <div class="uni-dash-label">${escapeHtml(w.config.command || w.config.content || '')}</div>`;
      // Drag
      card.addEventListener('dragstart', e => { e.dataTransfer.setData('text/plain', idx); card.classList.add('dragging'); });
      card.addEventListener('dragend', () => card.classList.remove('dragging'));
      card.addEventListener('dragover', e => { e.preventDefault(); card.classList.add('drag-over'); });
      card.addEventListener('dragleave', () => card.classList.remove('drag-over'));
      card.addEventListener('drop', e => {
        e.preventDefault(); card.classList.remove('drag-over');
        const from = parseInt(e.dataTransfer.getData('text/plain'));
        const to = idx;
        if (from !== to) { const [moved] = items.splice(from, 1); items.splice(to, 0, moved); renderEditCards(); }
      });
      // Edit
      card.querySelector('.dash-card-edit').addEventListener('click', () => showWidgetModal(w, updated => { items[idx] = updated; renderEditCards(); }));
      // Delete
      card.querySelector('.dash-card-delete').addEventListener('click', () => { items.splice(idx, 1); renderEditCards(); });
      grid.appendChild(card);
    });
  }
  renderEditCards();

  // Add
  toolbar.querySelector('.dash-add-btn').addEventListener('click', () => {
    const newWidget = { widget_type: 'stat', config: { label: 'Новый', color: 'blue', command: '', valuePath: '', emptyValue: '0' } };
    showWidgetModal(newWidget, saved => { items.push(saved); renderEditCards(); });
  });
  // Done
  toolbar.querySelector('.dash-done-btn').addEventListener('click', async () => {
    const payload = items.map((w, i) => ({ widget_type: w.widget_type, config: w.config, position: i }));
    await invoke('save_dashboard_widgets', { tabId, widgets: JSON.stringify(payload) }).catch(e => alert(e));
    renderDashboard(paneEl, tabId);
  });
}

function showWidgetModal(widget, onSave) {
  const c = widget.config;
  const curType = WIDGET_TYPES.find(t => t.value === widget.widget_type)?.label || 'Число';
  const colorDots = COLORS.map(cl => {
    const sel = cl.name === (c.color || 'blue');
    return `<span class="wm-dot${sel ? ' selected' : ''}" data-color="${cl.name}" style="background:${cl.hex}">${sel ? '<svg class="wm-dot-check" width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="3,8 7,12 13,4"/></svg>' : ''}</span>`;
  }).join('');
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal modal-compact">
    <div class="modal-title">Настройка виджета</div>
    <div class="form-group"><label class="form-label">Название</label><input class="form-input wm-label" value="${escapeHtml(c.label || '')}"></div>
    <div class="form-row">
      <div class="form-group" style="flex:1"><label class="form-label">Тип</label><div class="wm-dd" data-value="${widget.widget_type}"><div class="form-input wm-dd-btn">${curType}<svg class="wm-dd-chevron" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><polyline points="4,6 8,10 12,6"/></svg></div><div class="wm-dd-list">${WIDGET_TYPES.map(t => `<div class="wm-dd-opt${t.value === widget.widget_type ? ' sel' : ''}" data-v="${t.value}">${t.label}</div>`).join('')}</div></div></div>
      <div class="form-group" style="flex:1"><label class="form-label">Цвет</label><div class="wm-dots">${colorDots}</div></div>
    </div>
    <div class="form-group"><label class="form-label">Команда</label><input class="form-input wm-command" value="${escapeHtml(c.command || '')}" placeholder="get_job_stats"></div>
    <details class="wm-advanced"><summary class="wm-advanced-toggle">Расширенные настройки</summary>
      <div class="wm-advanced-body">
        <div class="form-row">
          <div class="form-group" style="flex:1"><label class="form-label">Путь к данным</label><input class="form-input wm-path" value="${escapeHtml(c.valuePath || '')}" placeholder="total"></div>
          <div class="form-group" style="flex:1"><label class="form-label">Суффикс</label><input class="form-input wm-suffix" value="${escapeHtml(c.suffix || '')}" placeholder="ч, кг..."></div>
        </div>
        <div class="form-group"><label class="form-label">Args (JSON)</label><input class="form-input wm-args" value='${escapeHtml(JSON.stringify(c.commandArgs || {}))}' placeholder="{}"></div>
      </div>
    </details>
    <div class="modal-actions">
      <button class="btn-secondary wm-cancel">Отмена</button>
      <button class="btn-primary wm-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('.wm-cancel').addEventListener('click', () => overlay.remove());
  // Dropdown
  const dd = overlay.querySelector('.wm-dd');
  dd.querySelector('.wm-dd-btn').addEventListener('click', () => dd.classList.toggle('open'));
  dd.querySelectorAll('.wm-dd-opt').forEach(opt => opt.addEventListener('click', () => {
    dd.dataset.value = opt.dataset.v;
    dd.querySelector('.wm-dd-btn').textContent = opt.textContent;
    dd.querySelectorAll('.wm-dd-opt').forEach(o => o.classList.remove('sel'));
    opt.classList.add('sel');
    dd.classList.remove('open');
  }));
  overlay.addEventListener('click', e => { if (!dd.contains(e.target)) dd.classList.remove('open'); });
  // Color dots
  overlay.querySelectorAll('.wm-dot').forEach(dot => dot.addEventListener('click', () => {
    overlay.querySelectorAll('.wm-dot').forEach(d => { d.classList.remove('selected'); d.innerHTML = ''; });
    dot.classList.add('selected');
    dot.innerHTML = '<svg class="wm-dot-check" width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="#fff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="3,8 7,12 13,4"/></svg>';
  }));
  // Save
  overlay.querySelector('.wm-save').addEventListener('click', () => {
    let args = {};
    try { args = JSON.parse(overlay.querySelector('.wm-args').value || '{}'); } catch {}
    onSave({
      ...widget,
      widget_type: dd.dataset.value,
      config: { ...c,
        label: overlay.querySelector('.wm-label').value,
        color: overlay.querySelector('.wm-dot.selected')?.dataset.color || 'blue',
        command: overlay.querySelector('.wm-command').value,
        commandArgs: args,
        valuePath: overlay.querySelector('.wm-path').value,
        suffix: overlay.querySelector('.wm-suffix').value,
      },
    });
    overlay.remove();
  });
}
