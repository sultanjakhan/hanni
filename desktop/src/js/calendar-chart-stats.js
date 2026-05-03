// Calendar chart — pure data/stats helpers.
// No DOM. Only fetches via invoke() and computes per-day metrics + summary.

import { invoke } from './state.js';

export const RU_DOW = ['Вс', 'Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб'];

export function todayStr() {
  const t = new Date();
  return `${t.getFullYear()}-${String(t.getMonth()+1).padStart(2,'0')}-${String(t.getDate()).padStart(2,'0')}`;
}

export function computeDays(n, endDate = todayStr()) {
  const out = [];
  const end = new Date(endDate + 'T12:00:00');
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(end); d.setDate(end.getDate() - i);
    out.push(`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`);
  }
  return out;
}

export function scheduleMatches(sch, dateStr) {
  if (!sch.is_active) return false;
  if (sch.until_date && dateStr > sch.until_date) return false;
  if (sch.frequency === 'daily') return true;
  const dow = (new Date(dateStr + 'T12:00:00').getDay()) || 7;
  if (sch.frequency === 'weekly' || sch.frequency === 'custom') {
    const days = sch.frequency_days ? sch.frequency_days.split(',').map(Number) : [1];
    return days.includes(dow);
  }
  return false;
}

// 'none' = ничего не запланировано; 'zero' = запланировано но 0%; 'low/mid/high' по %
export function tierOf(pct, planned) {
  if (!planned) return 'none';
  if (pct === 0) return 'zero';
  if (pct < 30) return 'low';
  if (pct < 70) return 'mid';
  return 'high';
}

function passType(kind, types) { return types.includes(kind); }
function passCat(kind, cat, catFilter) {
  if (!catFilter) return true;
  return catFilter.includes(`${kind}:${cat || 'general'}`);
}

export async function loadChartData(days, filters) {
  const types = filters.types || ['event', 'task', 'schedule'];
  const cats = filters.categories || null;
  const [events, tasks, schedules, completionsArr] = await Promise.all([
    invoke('get_all_events').catch(() => []),
    invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
    Promise.all(days.map(d => invoke('get_schedule_completions', { date: d }).catch(() => []))),
  ]);

  // collect all available categories for filter UI
  const availableCats = new Set();
  for (const e of events || []) if (passType('event', ['event','task','schedule'])) availableCats.add(`event:${e.category || 'general'}`);
  for (const s of schedules || []) if (s.is_active) availableCats.add(`schedule:${s.category || 'other'}`);
  // tasks have no category in DB — group as task:all
  if ((tasks || []).length) availableCats.add('task:all');

  const eventsByDay = new Map(); const tasksByDay = new Map();
  for (const e of events || []) {
    if (!e.date) continue;
    if (!passType('event', types)) continue;
    if (!passCat('event', e.category || 'general', cats)) continue;
    if (!eventsByDay.has(e.date)) eventsByDay.set(e.date, []);
    eventsByDay.get(e.date).push(e);
  }
  for (const t of tasks || []) {
    if (!t.due_date) continue;
    if (!passType('task', types)) continue;
    if (!passCat('task', 'all', cats)) continue;
    if (!tasksByDay.has(t.due_date)) tasksByDay.set(t.due_date, []);
    tasksByDay.get(t.due_date).push(t);
  }
  const filteredScheds = (schedules || []).filter(s =>
    passType('schedule', types) && passCat('schedule', s.category || 'other', cats)
  );

  const data = days.map((day, i) => {
    const dayEvents = eventsByDay.get(day) || [];
    const dayTasks = tasksByDay.get(day) || [];
    const daySchedules = filteredScheds.filter(s => scheduleMatches(s, day));
    const compIds = new Set((completionsArr[i] || []).filter(c => c.completed).map(c => c.schedule_id));
    const ev = { done: dayEvents.filter(e => e.completed).length, total: dayEvents.length };
    const tk = { done: dayTasks.filter(t => t.status === 'done').length, total: dayTasks.length };
    const sc = { done: daySchedules.filter(s => compIds.has(s.id)).length, total: daySchedules.length };
    const planned = ev.total + tk.total + sc.total;
    const done = ev.done + tk.done + sc.done;
    const pct = planned > 0 ? Math.round(done / planned * 100) : null;
    return { day, planned, done, pct, ev, tk, sc, tier: tierOf(pct, planned) };
  });

  return { data, availableCats: [...availableCats].sort() };
}

// Streak ending today: consecutive recent days with pct >= threshold (planned must be > 0)
export function computeStreak(data, threshold = 50) {
  let streak = 0;
  for (let i = data.length - 1; i >= 0; i--) {
    const d = data[i];
    if (d.planned > 0 && d.pct >= threshold) streak++;
    else break;
  }
  return streak;
}

export function computeMissed(data) {
  return data.filter(d => d.planned > 0 && d.pct === 0).length;
}

export function computeAvg(data) {
  const filled = data.filter(d => d.planned > 0);
  if (!filled.length) return null;
  return Math.round(filled.reduce((s, d) => s + d.pct, 0) / filled.length);
}

export async function computeTrend(currentData, period, filters) {
  // load previous period of equal length, ending one day before current period starts
  const firstCurrent = currentData[0]?.day;
  if (!firstCurrent) return null;
  const prevEnd = new Date(firstCurrent + 'T12:00:00');
  prevEnd.setDate(prevEnd.getDate() - 1);
  const prevEndStr = `${prevEnd.getFullYear()}-${String(prevEnd.getMonth()+1).padStart(2,'0')}-${String(prevEnd.getDate()).padStart(2,'0')}`;
  const prevDays = computeDays(period, prevEndStr);
  const { data: prevData } = await loadChartData(prevDays, filters);
  const curAvg = computeAvg(currentData);
  const prevAvg = computeAvg(prevData);
  if (curAvg == null || prevAvg == null) return null;
  return curAvg - prevAvg; // signed delta in percentage points
}
