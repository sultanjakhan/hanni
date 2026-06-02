// calendar-event-popover.js — read-only "what was done" popover for a day-view
// event. Opened on click of a timeline event block (incl. auto Health
// activities like Сон/Прогулка). Edit stays behind an explicit button so a
// click no longer jumps straight into the edit modal.

import { escapeHtml } from './utils.js';

const SOURCE_LABEL = {
  manual: 'Вручную',
  apple: 'Apple Calendar',
  auto_health: 'Apple Health',
  google: 'Google Calendar',
};

const fmtDur = (min) => {
  min = Math.round(min || 0);
  if (min < 1) return '<1м';
  if (min < 60) return `${min}м`;
  const h = Math.floor(min / 60), m = min % 60;
  return m ? `${h}ч ${m}м` : `${h}ч`;
};

const hhmm = (mins) =>
  `${String(Math.floor(mins / 60) % 24).padStart(2, '0')}:${String(mins % 60).padStart(2, '0')}`;

export function closeEventPopover() {
  document.querySelectorAll('.cal-event-pop').forEach(p => p.remove());
}

export function showEventPopover(event, x, y, { onEdit } = {}) {
  closeEventPopover();
  const isManual = !event.source || event.source === 'manual';
  const dur = event.duration_minutes || 0;

  let timeRow = '';
  if (event.time) {
    const [h, m] = event.time.split(':').map(Number);
    timeRow = `<div class="cal-pop-row">🕐 ${event.time} – ${hhmm(h * 60 + m + dur)}</div>`;
  }
  // auto_health duration is the real tracked value, not a plan.
  const durLabel = event.source === 'auto_health' ? 'Факт' : 'Длительность';
  const srcLabel = SOURCE_LABEL[event.source] || (isManual ? 'Вручную' : event.source);
  const doneRow = event.completed
    ? '<div class="cal-pop-row cal-pop-done">✓ Выполнено</div>'
    : '<div class="cal-pop-row cal-pop-muted">○ Не отмечено</div>';
  const descRow = event.description
    ? `<div class="cal-pop-desc">${escapeHtml(event.description)}</div>` : '';
  const editBtn = (isManual && onEdit)
    ? '<button type="button" class="btn-secondary cal-pop-edit">Редактировать</button>' : '';

  const pop = document.createElement('div');
  pop.className = 'inline-dropdown cal-event-pop';
  pop.style.left = x + 'px';
  pop.style.top = y + 'px';
  pop.innerHTML = `
    <div class="cal-pop-title">${escapeHtml(event.title || 'Событие')}</div>
    ${timeRow}
    ${dur ? `<div class="cal-pop-row">⏱ ${durLabel}: ${fmtDur(dur)}</div>` : ''}
    <div class="cal-pop-row cal-pop-muted">${escapeHtml(srcLabel)}</div>
    ${doneRow}
    ${descRow}
    ${editBtn}`;
  document.body.appendChild(pop);

  // Keep the card on screen.
  const rect = pop.getBoundingClientRect();
  if (rect.right > window.innerWidth) pop.style.left = Math.max(8, x - rect.width) + 'px';
  if (rect.bottom > window.innerHeight) pop.style.top = Math.max(8, y - rect.height) + 'px';

  pop.querySelector('.cal-pop-edit')?.addEventListener('click', () => {
    closeEventPopover();
    onEdit?.();
  });

  // Dismiss on outside click / Esc.
  setTimeout(() => document.addEventListener('mousedown', function out(ev) {
    if (!pop.contains(ev.target)) { pop.remove(); document.removeEventListener('mousedown', out); }
  }), 10);
  document.addEventListener('keydown', function esc(ev) {
    if (ev.key === 'Escape') { closeEventPopover(); document.removeEventListener('keydown', esc); }
  });
}
