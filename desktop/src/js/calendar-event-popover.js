// calendar-event-popover.js — Google/Notion-style detail card for day-view
// items. Opened on click of an event block, a completed-task box, or a timer
// block. Read-only by default; completed tasks get an "Отменить" action and
// manual events an "Редактировать" button. showPagerPopover flips through a
// group with ‹ › / arrow keys.

import { escapeHtml } from './utils.js';
import { invoke } from './state.js';

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

const CLOSE_BTN = '<button type="button" class="cal-pop-close" aria-label="Закрыть">×</button>';
const rowsToHtml = (rows) => (rows || []).map(r =>
  `<div class="cal-pop-row${r.muted ? ' cal-pop-muted' : ''}${r.done ? ' cal-pop-done' : ''}">${escapeHtml(r.text)}</div>`).join('');

export function closeEventPopover() {
  document.querySelectorAll('.cal-event-pop').forEach(p => p.remove());
}

// Create the card, position it on screen, wire close × / Esc / outside-click.
function mountPopover(innerHtml, x, y, accent) {
  closeEventPopover();
  const pop = document.createElement('div');
  pop.className = 'inline-dropdown cal-event-pop';
  if (accent) pop.style.setProperty('--cal-pop-accent', accent);
  pop.style.left = x + 'px';
  pop.style.top = y + 'px';
  pop.innerHTML = innerHtml;
  document.body.appendChild(pop);

  // Keep the card on screen.
  const rect = pop.getBoundingClientRect();
  if (rect.right > window.innerWidth) pop.style.left = Math.max(8, x - rect.width) + 'px';
  if (rect.bottom > window.innerHeight) pop.style.top = Math.max(8, y - rect.height) + 'px';

  // Dismiss on outside click / Esc.
  setTimeout(() => document.addEventListener('mousedown', function out(ev) {
    if (!pop.contains(ev.target)) { pop.remove(); document.removeEventListener('mousedown', out); }
  }), 10);
  document.addEventListener('keydown', function esc(ev) {
    if (ev.key === 'Escape') { closeEventPopover(); document.removeEventListener('keydown', esc); }
  });
  return pop;
}

export function showEventPopover(event, x, y, { onEdit } = {}) {
  const isManual = !event.source || event.source === 'manual';
  const dur = event.duration_minutes || 0;

  let timeRow = '';
  if (event.time) {
    const [h, m] = event.time.split(':').map(Number);
    timeRow = `<div class="cal-pop-row">🕐 ${event.time} – ${hhmm(h * 60 + m + dur)}</div>`;
  }
  const durLabel = event.source === 'auto_health' ? 'Факт' : 'Длительность';
  const srcLabel = SOURCE_LABEL[event.source] || (isManual ? 'Вручную' : event.source);
  const doneRow = event.completed
    ? '<div class="cal-pop-row cal-pop-done">✓ Выполнено</div>'
    : '<div class="cal-pop-row cal-pop-muted">○ Не отмечено</div>';
  const descRow = event.description
    ? `<div class="cal-pop-desc">${escapeHtml(event.description)}</div>` : '';
  const editBtn = (isManual && onEdit)
    ? '<button type="button" class="btn-secondary cal-pop-edit">Редактировать</button>' : '';

  const pop = mountPopover(`
    ${CLOSE_BTN}
    <div class="cal-pop-title">${escapeHtml(event.title || 'Событие')}</div>
    ${timeRow}
    ${dur ? `<div class="cal-pop-row">⏱ ${durLabel}: ${fmtDur(dur)}</div>` : ''}
    <div class="cal-pop-row cal-pop-muted">${escapeHtml(srcLabel)}</div>
    ${doneRow}
    ${descRow}
    ${editBtn ? `<div class="cal-pop-actions">${editBtn}</div>` : ''}`, x, y, event.color);

  pop.querySelector('.cal-pop-close')?.addEventListener('click', () => pop.remove());
  pop.querySelector('.cal-pop-edit')?.addEventListener('click', () => {
    closeEventPopover();
    onEdit?.();
  });
}

// Read-only details for a timer block: { title, subtitle, rows, accent }.
export function showTimelinePopover(x, y, { title, subtitle, rows = [], accent } = {}) {
  const subRow = subtitle
    ? `<div class="cal-pop-row cal-pop-muted">${escapeHtml(subtitle)}</div>` : '';
  const pop = mountPopover(`
    ${CLOSE_BTN}
    <div class="cal-pop-title">${escapeHtml(title || 'Активность')}</div>
    ${subRow}
    ${rowsToHtml(rows)}`, x, y, accent);
  pop.querySelector('.cal-pop-close')?.addEventListener('click', () => pop.remove());
}

// Click-through card for a group of completions (and folded events). Each item:
// { title, subtitle, rows, accent?, toggle?: { scheduleId, date, done } }. An
// item with `toggle` shows a "Отметить"/"Отменить" button that flips its
// completion and refreshes the calendar.
export function showPagerPopover(items, x, y) {
  if (!items || !items.length) return;
  let idx = 0;
  const pageHtml = (i) => {
    const it = items[i] || {};
    const nav = items.length > 1 ? `<div class="cal-pop-pager">
      <button type="button" class="cal-pop-nav" data-dir="-1" aria-label="Назад">‹</button>
      <span class="cal-pop-count">${i + 1} / ${items.length}</span>
      <button type="button" class="cal-pop-nav" data-dir="1" aria-label="Вперёд">›</button>
    </div>` : '';
    const sub = it.subtitle ? `<div class="cal-pop-row cal-pop-muted">${escapeHtml(it.subtitle)}</div>` : '';
    const tg = it.toggle;
    const actBtn = tg
      ? `<button type="button" class="btn-secondary ${tg.done ? 'cal-pop-undo' : 'cal-pop-mark'}">${tg.done ? 'Отменить выполнение' : 'Отметить выполненным'}</button>`
      : '';
    return `${CLOSE_BTN}${nav}<div class="cal-pop-title">${escapeHtml(it.title || '')}</div>${sub}${rowsToHtml(it.rows)}${actBtn ? `<div class="cal-pop-actions">${actBtn}</div>` : ''}`;
  };
  const pop = mountPopover(pageHtml(0), x, y, (items[0] || {}).accent);
  const bind = () => {
    const it = items[idx] || {};
    if (it.accent) pop.style.setProperty('--cal-pop-accent', it.accent);
    pop.querySelector('.cal-pop-close')?.addEventListener('click', () => pop.remove());
    pop.querySelectorAll('.cal-pop-nav').forEach(b => b.addEventListener('click', (e) => {
      e.stopPropagation();
      idx = (idx + Number(b.dataset.dir) + items.length) % items.length;
      pop.innerHTML = pageHtml(idx); bind();
    }));
    pop.querySelector('.cal-pop-undo, .cal-pop-mark')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const t = it.toggle; pop.remove();
      try { await invoke('toggle_schedule_completion', { scheduleId: t.scheduleId, date: t.date }); } catch (_) {}
      window.dispatchEvent(new Event('hanni:calendar-refresh'));
    });
  };
  bind();
  document.addEventListener('keydown', function onKey(e) {
    if (!pop.isConnected) { document.removeEventListener('keydown', onKey); return; }
    if (e.key === 'ArrowLeft') { idx = (idx - 1 + items.length) % items.length; pop.innerHTML = pageHtml(idx); bind(); }
    else if (e.key === 'ArrowRight') { idx = (idx + 1) % items.length; pop.innerHTML = pageHtml(idx); bind(); }
  });
}
