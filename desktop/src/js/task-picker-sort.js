// task-picker-sort.js — Composite importance ranking for the "Запустить таск" picker.
// Pure logic, no DOM. Ranks startable tasks so the most relevant surfaces first
// and flags overdue ones. Inputs come from get_today_planned (events/schedules/notes).

// "HH:MM" → minutes from midnight, or null when absent/unparseable.
export function timeToMin(t) {
  if (!t || typeof t !== 'string') return null;
  const m = t.match(/^(\d{1,2}):(\d{2})/);
  if (!m) return null;
  return parseInt(m[1], 10) * 60 + parseInt(m[2], 10);
}

// A task counts as overdue only within this grace window past its time — beyond
// it the task is "missed for the day" and drops to its normal group, so early
// morning schedules don't scream "−14ч" all evening.
const OVERDUE_GRACE_MIN = 180;

// A task is overdue when its planned time is recently past and it isn't done.
export function isOverdue(item, nowMin) {
  if (item.completed || item.status_extra === 'done') return false;
  const t = timeToMin(item.planned_time);
  return t !== null && t < nowMin && (nowMin - t) <= OVERDUE_GRACE_MIN;
}

// Editable per-category weight (KV settings); missing → neutral 1.
function categoryWeight(item, weights) {
  const w = weights && weights[item.category];
  return typeof w === 'number' ? w : 1;
}

// Bonus for tasks happening soon: stronger as planned time approaches.
function soonBonus(item, nowMin) {
  const t = timeToMin(item.planned_time);
  if (t === null || t < nowMin) return 0;
  const mins = t - nowMin;
  if (mins <= 60) return 2;
  if (mins <= 180) return 1;
  return 0;
}

// Higher = more important. Combines explicit priority, category weight, proximity.
function score(item, weights, nowMin) {
  return (item.priority || 0) + categoryWeight(item, weights) + soonBonus(item, nowMin);
}

// Identity used by pins (matches the picker's source_type/source_id pair).
export function pinKey(item) {
  return `${item.source_type}:${item.source_id}`;
}

// Sort a flat list of startable items by composite importance and tag each with
// `_overdue` / `_pinned`. Order: pinned → overdue → composite score.
export function rankTasks(items, { nowMin, weights, pins = [] }) {
  const pinSet = new Set(pins);
  return items
    .map(it => ({ ...it, _overdue: isOverdue(it, nowMin), _pinned: pinSet.has(pinKey(it)) }))
    .sort((a, b) => {
      if (a._pinned !== b._pinned) return a._pinned ? -1 : 1;
      if (a._overdue !== b._overdue) return a._overdue ? -1 : 1;
      if (a._overdue && b._overdue) {
        return (timeToMin(a.planned_time) ?? 0) - (timeToMin(b.planned_time) ?? 0);
      }
      const sd = score(b, weights, nowMin) - score(a, weights, nowMin);
      if (sd !== 0) return sd;
      const ta = timeToMin(a.planned_time), tb = timeToMin(b.planned_time);
      if (ta !== tb) return (ta ?? 1e9) - (tb ?? 1e9);
      return (a.title || '').localeCompare(b.title || '');
    });
}

// The chain whose start should be recommended now: first unstarted auto-trigger
// chain (sleep_end = wake, or time). Manual chains aren't auto-recommended.
// Returns its chain id, or null. Use only when no active run owns the recommendation.
export function pickStartChainId(chains, runs) {
  const activeChainIds = new Set((runs || []).map(r => r.chain_id));
  const c = (chains || []).find(
    ch => (ch.trigger_type === 'sleep_end' || ch.trigger_type === 'time') && !activeChainIds.has(ch.id)
  );
  return c ? c.id : null;
}

// The single graph recommendation: top available task across active routine runs.
// Required steps outrank optional ones, then higher priority. Null when nothing active.
export function pickRecommendedTaskId(runs) {
  const rank = t => (t.requirement === 'required' ? 100 : 0) + (t.priority || 0);
  let best = null;
  for (const run of runs || []) {
    for (const t of run.tasks || []) {
      if (!best || rank(t) > rank(best)) best = t;
    }
  }
  return best ? best.id : null;
}

// Current local time as minutes from midnight.
export function nowMinutes() {
  const d = new Date();
  return d.getHours() * 60 + d.getMinutes();
}
