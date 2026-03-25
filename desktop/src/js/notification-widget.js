// ── notification-widget.js — Bell icon with live notification feed ──
import { invoke, listen } from './state.js';

let nwPanel = null;
let nwBadge = null;
let nwList = null;
let refreshTimer = null;

// Calculate "через X мин" from HH:MM string
function timeUntil(timeStr, nowStr) {
  const [h, m] = timeStr.split(':').map(Number);
  const [nh, nm] = nowStr.split(':').map(Number);
  const diff = (h * 60 + m) - (nh * 60 + nm);
  if (diff <= 0) return 'сейчас';
  if (diff < 60) return `через ${diff} мин`;
  const hrs = Math.floor(diff / 60);
  const mins = diff % 60;
  return mins > 0 ? `через ${hrs}ч ${mins}м` : `через ${hrs}ч`;
}

function renderItems(data) {
  const items = [];

  // Upcoming events with countdown
  for (const ev of data.upcoming || []) {
    const countdown = timeUntil(ev.time, data.now);
    items.push(`<div class="nw-item nw-event">
      <span class="nw-icon">📅</span>
      <span class="nw-text">${ev.title}</span>
      <span class="nw-time">${countdown}</span>
    </div>`);
  }

  // Overdue tasks
  for (const t of data.overdue || []) {
    items.push(`<div class="nw-item nw-overdue">
      <span class="nw-icon">⚠️</span>
      <span class="nw-text">${t.title}</span>
      <span class="nw-time">${t.due_date}</span>
    </div>`);
  }

  // Missed schedules
  for (const s of data.missed_schedules || []) {
    items.push(`<div class="nw-item nw-missed">
      <span class="nw-icon">◻</span>
      <span class="nw-text">${s.title}</span>
      <span class="nw-time">сегодня</span>
    </div>`);
  }

  // Footer
  if (data.done_today > 0) {
    items.push(`<div class="nw-footer">✅ Сделано сегодня: ${data.done_today}</div>`);
  }

  return items.length > 0 ? items.join('') : '<div class="nw-empty">Событий нет</div>';
}

async function refresh() {
  try {
    const data = await invoke('get_notifications');
    if (nwBadge) {
      nwBadge.textContent = data.total || '';
      nwBadge.classList.toggle('hidden', !data.total);
    }
    if (nwList) nwList.innerHTML = renderItems(data);
  } catch (_) {}
}

function toggle() {
  if (!nwPanel) return;
  const open = nwPanel.classList.toggle('hidden');
  if (!open) refresh();
}

export function initNotificationWidget() {
  const existing = document.getElementById('notification-widget');
  if (existing) existing.remove();

  const widget = document.createElement('div');
  widget.id = 'notification-widget';
  widget.className = 'notification-widget';
  widget.innerHTML = `
    <div class="nw-panel hidden" id="nw-panel">
      <div class="nw-header">Уведомления</div>
      <div class="nw-list" id="nw-list"></div>
    </div>
    <div class="nw-btn" id="nw-btn">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/>
        <path d="M13.73 21a2 2 0 0 1-3.46 0"/>
      </svg>
      <span class="nw-badge hidden" id="nw-badge"></span>
    </div>
  `;
  document.getElementById('content-area').appendChild(widget);

  nwPanel = document.getElementById('nw-panel');
  nwBadge = document.getElementById('nw-badge');
  nwList = document.getElementById('nw-list');

  document.getElementById('nw-btn').addEventListener('click', toggle);

  // Close on outside click
  document.addEventListener('click', (e) => {
    if (nwPanel && !nwPanel.classList.contains('hidden') && !widget.contains(e.target)) {
      nwPanel.classList.add('hidden');
    }
  });

  // Refresh on proactive message
  listen('proactive-message', () => setTimeout(refresh, 200));

  // Initial load + periodic refresh
  refresh();
  refreshTimer = setInterval(refresh, 60000);
}
