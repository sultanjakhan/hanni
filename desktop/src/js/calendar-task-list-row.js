// Calendar Day list — row rendering + section rendering + overdue formatting.
// Pure presentation. Wiring happens in calendar-task-list.js.

import { escapeHtml } from './utils.js';

export const SECTION_DEFS = [
  { key: 'overdue',  label: 'Просрочено', icon: '⚠️', cls: 'ctl-section-overdue' },
  { key: 'schedule', label: 'Расписание', icon: '🔁' },
  { key: 'event',    label: 'События',    icon: '📅' },
  { key: 'note',     label: 'Задачи',     icon: '📝' },
];

export function fmtOverdue(dueDate, today) {
  if (!dueDate) return '';
  const due = new Date(dueDate + 'T12:00:00');
  const now = new Date(today + 'T12:00:00');
  const diff = Math.round((now - due) / 86400000);
  if (diff <= 0) return '';
  if (diff === 1) return 'вчера';
  if (diff < 7) return `${diff} дн.`;
  if (diff < 30) return `${Math.round(diff / 7)} нед.`;
  return `${Math.round(diff / 30)} мес.`;
}

export function renderItemRow(item, dateStr) {
  const active = item.block?.is_active;
  const done = !!item.done;
  const pr = item.priority || 0;
  const target = item.targetMinutes || 0;
  const actual = item.actualMinutes || 0;
  const targetReached = target > 0 && actual >= target;
  const cls = ['ctl-row', done && 'ctl-done', active && 'ctl-active', pr > 0 && `ctl-pr-${pr}`, item.overdueDate && 'ctl-overdue'].filter(Boolean).join(' ');
  const durBadge = active && item.block?.duration_minutes ? `<span class="ctl-duration">${item.block.duration_minutes} мин</span>` : '';
  const progressBadge = target > 0
    ? `<span class="ctl-progress${targetReached ? ' ctl-progress-done' : ''}">${actual} / ${target} мин</span>`
    : '';
  const overdueBadge = item.overdueDate
    ? `<span class="ctl-overdue-badge" title="Срок: ${escapeHtml(item.overdueDate)}">${fmtOverdue(item.overdueDate, dateStr)}</span>`
    : '';
  // Show ▶ when there is room left: not done OR (has target and not yet reached)
  const showStart = !active && (!done || (target > 0 && !targetReached));
  const trackBtns = active
    ? `<button class="ctl-track ctl-pause" data-ctl-pause="${item.block.id}" title="Пауза">⏸</button>
       <button class="ctl-track ctl-finish" data-ctl-finish="${item.block.id}" title="Готово">✓</button>`
    : (showStart ? `<button class="ctl-track ctl-start" data-ctl-start title="${done && target > 0 ? 'Продолжить' : 'Запустить'}">▶</button>` : '');
  const prDot = pr > 0 ? `<span class="ctl-priority ctl-priority-${pr}" title="${pr === 2 ? 'Критическая' : 'Важная'}"></span>` : '<span class="ctl-priority"></span>';
  return `<div class="${cls}" data-kind="${item.kind}" data-id="${item.id}" data-date="${dateStr || ''}" data-target="${target}" data-actual="${actual}">
    ${prDot}
    <div class="ctl-check${done ? ' done' : ''}" data-ctl-check>${done ? '✓' : ''}</div>
    <span class="ctl-icon">${item.icon}</span>
    <span class="ctl-title">${escapeHtml(item.title)}</span>
    ${overdueBadge}
    ${progressBadge}
    ${durBadge}
    ${trackBtns}
  </div>`;
}

export function renderSection(def, items, date) {
  if (!items.length) return '';
  const rows = items.map(it => renderItemRow(it, date)).join('');
  return `<div class="ctl-section ${def.cls || ''}">
    <div class="ctl-section-title">${def.icon} ${def.label} <span class="ctl-section-count">${items.length}</span></div>
    ${rows}
  </div>`;
}
