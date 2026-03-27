// ── tab-body.js — Body tab: 3D skeleton viewer with records ──
import { invoke } from './state.js';
import { initViewer, disposeViewer, getZoneInfo, setZoneHighlights } from './body-viewer.js';
import { showBodyContextMenu } from './body-context-menu.js';

let currentZone = null;

export async function loadBody() {
  const el = document.getElementById('body-content');
  if (!el) return;
  await loadBodyInline(el);
}

/** Render body viewer inline into any container element */
export async function loadBodyInline(el) {
  disposeViewer();
  el.innerHTML = `
    <div class="body-layout">
      <div class="body-toolbar">
        <div class="body-layer-toggles">
          <button class="body-layer-btn active" data-layer="bone">Скелет</button>
          <button class="body-layer-btn" data-layer="muscle" disabled>Мышцы</button>
          <button class="body-layer-btn" data-layer="organ" disabled>Органы</button>
        </div>
      </div>
      <div class="body-main">
        <div class="body-viewer-container" id="body-viewer"></div>
        <div class="body-panel hidden" id="body-panel">
          <div class="body-panel-header">
            <span class="body-panel-zone-name">—</span>
            <button class="body-panel-close">&times;</button>
          </div>
          <div class="body-panel-content"></div>
        </div>
      </div>
    </div>`;

  const viewerEl = el.querySelector('#body-viewer');
  const panelEl = el.querySelector('#body-panel');

  initViewer(viewerEl, {
    onSelect: (obj) => {
      if (!obj) { hidePanel(panelEl); currentZone = null; return; }
      const info = getZoneInfo(obj);
      currentZone = info;
      showPanel(panelEl, info);
    },
    onContextMenu: (x, y, obj) => {
      const info = getZoneInfo(obj);
      currentZone = info;
      showBodyContextMenu(x, y, info.zone, info.label, {
        onHistory: () => showPanel(panelEl, info),
        onSaved: () => { refreshPanel(panelEl); refreshHighlights(); },
      });
    },
  });

  el.querySelector('.body-panel-close').onclick = () => { hidePanel(panelEl); currentZone = null; };
  refreshHighlights();
}

function showPanel(panelEl, info) {
  panelEl.classList.remove('hidden');
  panelEl.querySelector('.body-panel-zone-name').textContent = info.label || info.zone;
  refreshPanel(panelEl);
}

function hidePanel(panelEl) {
  panelEl.classList.add('hidden');
}

async function refreshPanel(panelEl) {
  if (!currentZone) return;
  const content = panelEl.querySelector('.body-panel-content');
  content.innerHTML = '<div class="body-panel-loading">Загрузка...</div>';
  try {
    const records = await invoke('get_body_records', { zone: currentZone.zone });
    renderRecords(content, records);
  } catch (err) {
    content.innerHTML = `<div class="body-panel-empty">Ошибка: ${err}</div>`;
  }
}

function renderRecords(container, records) {
  if (!records || records.length === 0) {
    container.innerHTML = '<div class="body-panel-empty">Нет записей. ПКМ на модели для добавления.</div>';
    return;
  }
  const icons = { pain: '🔴', workout: '💪', goal: '🎯', treatment: '💊', measurement: '📏', note: '📝' };
  container.innerHTML = records.map(r => `
    <div class="body-record" data-id="${r.id}">
      <div class="body-record-head">
        <span>${icons[r.record_type] || '📋'} ${recordLabel(r)}</span>
        <span class="body-record-date">${r.date}</span>
      </div>
      ${r.note ? `<div class="body-record-note">${escHtml(r.note)}</div>` : ''}
      <button class="body-record-del" data-id="${r.id}" title="Удалить">&times;</button>
    </div>
  `).join('');

  container.querySelectorAll('.body-record-del').forEach(btn => {
    btn.onclick = async (e) => {
      e.stopPropagation();
      await invoke('delete_body_record', { id: parseInt(btn.dataset.id) });
      refreshPanel(container.closest('.body-panel'));
      refreshHighlights();
    };
  });
}

function recordLabel(r) {
  switch (r.record_type) {
    case 'pain': return `Боль ${r.intensity || '?'}/10 (${r.pain_type || '?'})`;
    case 'workout': return 'Тренировка';
    case 'goal': return `Цель: ${r.goal_type || '?'}`;
    case 'treatment': return `Лечение: ${r.treatment_type || '?'}`;
    case 'measurement': return `Замер: ${r.value ?? '?'} ${r.unit || ''}`;
    case 'note': return 'Заметка';
    default: return r.record_type;
  }
}

async function refreshHighlights() {
  try {
    const summary = await invoke('get_body_zones_summary');
    const highlights = new Map();
    for (const s of summary) {
      if (s.record_type === 'pain') highlights.set(s.zone, 0xff4444);
      else if (s.record_type === 'goal' && !highlights.has(s.zone)) highlights.set(s.zone, 0x44aaff);
      else if (s.record_type === 'workout' && !highlights.has(s.zone)) highlights.set(s.zone, 0xff8800);
    }
    setZoneHighlights(highlights);
  } catch {}
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}
