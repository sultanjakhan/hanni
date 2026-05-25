// Day-grid overlay: paint completed timeline_blocks as absolutely-positioned
// rectangles spanning their real start..end time. Also marks free gaps and
// highlights blocks that conflict with planned events.

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

// Hour height defaults to 48px; caller passes the current zoom value.
const DEFAULT_HOUR_PX = 48;

const SOURCE_COLOR = { event: '#60a5fa', schedule: '#c084fc', note: '#4ade80', manual: '#94a3b8' };
const SOURCE_BG    = { event: '#dbeafe', schedule: '#f3e8ff', note: '#dcfce7', manual: '#f1f5f9' };
const SCH_CAT_ICONS = { health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡', practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽' };

// Extract HH:MM from a stored timestamp. completed_at is RFC3339 from
// chrono::Local — pull the local clock fields out without TZ math.
function timestampToHM(ts) {
  if (!ts || typeof ts !== 'string') return null;
  const m = ts.match(/T(\d{2}):(\d{2})/);
  if (!m) return null;
  return `${m[1]}:${m[2]}`;
}

const parseHM = (s) => {
  if (!s || typeof s !== 'string' || !/^\d{1,2}:\d{2}/.test(s)) return null;
  const [h, m] = s.split(':').map(Number);
  return h * 60 + m;
};
const fmtDur = (min) => {
  if (min < 1) return '<1м';
  if (min < 60) return `${min}м`;
  const h = Math.floor(min / 60), m = min % 60;
  return m ? `${h}ч ${m}м` : `${h}ч`;
};

async function loadSourceTitles(blocks) {
  // Pull event/note/schedule lookups in one shot. Used to give each block a real label.
  const needsEvent = blocks.some(b => b.source_type === 'event');
  const needsNote  = blocks.some(b => b.source_type === 'note');
  const needsSched = blocks.some(b => b.source_type === 'schedule');
  const [events, notes, scheds] = await Promise.all([
    needsEvent ? invoke('get_all_events').catch(() => []) : Promise.resolve([]),
    needsNote  ? invoke('get_notes', { filter: 'tasks', search: null }).catch(() => []) : Promise.resolve([]),
    needsSched ? invoke('get_schedules', { category: null }).catch(() => []) : Promise.resolve([]),
  ]);
  const idx = new Map();
  for (const e of events || []) idx.set(`event:${e.id}`, e.title || '');
  for (const n of notes || []) idx.set(`note:${n.id}`, n.title || '');
  for (const s of scheds || []) idx.set(`schedule:${s.id}`, s.title || '');
  return idx;
}

function blockHtml(block, label, conflict) {
  const colour = SOURCE_COLOR[block.source_type] || block.type_color || SOURCE_COLOR.manual;
  const bg = SOURCE_BG[block.source_type] || SOURCE_BG.manual;
  const icon = block.type_icon || '⏱';
  const start = block.start_time.slice(0, 5);
  const end = block.end_time.slice(0, 5);
  const tip = `${label || 'Активность'} · ${start}–${end} · ${fmtDur(block.duration_minutes || 0)}${conflict ? ' · ⚠️ конфликт с встречей' : ''}`;
  const cls = ['day-tl-block', conflict && 'day-tl-conflict'].filter(Boolean).join(' ');
  return `<div class="${cls}" style="--tl-color:${colour};--tl-bg:${bg};" title="${escapeHtml(tip)}">
    <div class="day-tl-block-head">
      <span class="day-tl-block-icon">${icon}</span>
      <span class="day-tl-block-label">${escapeHtml(label || 'Активность')}</span>
      ${conflict ? '<span class="day-tl-block-warn" title="конфликт с встречей">⚠️</span>' : ''}
    </div>
    <div class="day-tl-block-foot">${start}–${end} · ${fmtDur(block.duration_minutes || 0)}</div>
  </div>`;
}

function gapHtml(startMin, endMin, minToPx) {
  const dur = endMin - startMin;
  return `<div class="day-tl-gap" style="top:${minToPx(startMin)}px; height:${minToPx(dur)}px;">
    <span class="day-tl-gap-label">${fmtDur(dur)} свободно</span>
  </div>`;
}

export async function injectTimelineOverlay(rootEl, date, plannedEvents = [], hourPx = DEFAULT_HOUR_PX) {
  if (!rootEl) return;
  const minToPx = (m) => Math.round(m * (hourPx / 60));
  const blocks = await invoke('get_timeline_blocks', { date }).catch(() => []);
  const completed = (blocks || []).filter(b => !b.is_active && b.start_time && b.end_time && b.duration_minutes > 0);
  const titles = await loadSourceTitles(completed);

  // Build event-time intervals for conflict detection
  const eventIntervals = (plannedEvents || [])
    .filter(e => e.time && e.duration_minutes)
    .map(e => {
      const s = parseHM(e.time); if (s == null) return null;
      return { startMin: s, endMin: s + (e.duration_minutes || 60) };
    })
    .filter(Boolean);
  const isConflict = (b) => {
    const s = parseHM(b.start_time); const en = parseHM(b.end_time);
    if (s == null || en == null) return false;
    return eventIntervals.some(iv => s < iv.endMin && en > iv.startMin);
  };

  // Container fills the full day height of .day-timeline (24 * hourPx).
  const layer = document.createElement('div');
  layer.className = 'day-tl-layer';
  layer.style.height = `${24 * hourPx}px`;

  // A tracked block that exactly matches a planned event (same start + end) is
  // the same activity drawn twice — keep the planned event, drop the overlay.
  const plannedKeys = new Set(eventIntervals.map(iv => `${iv.startMin}:${iv.endMin}`));

  // Decorate with parsed minutes + sort by start_time for cluster building
  const positioned = completed
    .map(b => ({ block: b, startMin: parseHM(b.start_time), endMin: parseHM(b.end_time) }))
    .filter(x => x.startMin != null && x.endMin != null && x.endMin > x.startMin)
    .filter(x => !plannedKeys.has(`${x.startMin}:${x.endMin}`))
    .sort((a, b) => a.startMin - b.startMin || a.endMin - b.endMin);

  // Group into overlap clusters: a new cluster starts when current block's start
  // is past the cluster's max-end so far (no overlap with anything before).
  const clusters = [];
  let cur = [];
  let curMaxEnd = -1;
  for (const p of positioned) {
    if (cur.length > 0 && p.startMin >= curMaxEnd) {
      clusters.push(cur); cur = []; curMaxEnd = -1;
    }
    cur.push(p);
    if (p.endMin > curMaxEnd) curMaxEnd = p.endMin;
  }
  if (cur.length) clusters.push(cur);

  // Within each cluster, greedily assign column; total cols = max cols used.
  for (const cluster of clusters) {
    const colEnds = []; // colEnds[i] = endMin of last block in column i
    for (const p of cluster) {
      let col = colEnds.findIndex(end => end <= p.startMin);
      if (col === -1) { col = colEnds.length; colEnds.push(p.endMin); }
      else colEnds[col] = p.endMin;
      p.col = col;
    }
    const totalCols = colEnds.length;
    for (const p of cluster) p.totalCols = totalCols;
  }

  // Render blocks as absolutely-positioned rectangles, side-by-side when overlapping
  for (const p of positioned) {
    const top = minToPx(p.startMin);
    const height = Math.max(20, minToPx(p.endMin - p.startMin));
    const label = titles.get(`${p.block.source_type}:${p.block.source_id}`) || p.block.type_name || '';
    const wrap = document.createElement('div');
    wrap.className = 'day-tl-block-wrap';
    if (p.totalCols > 1) {
      // Split available width into N columns. CSS handles the math via custom
      // props. Cap at 4 columns so a pathological pile-up stays readable
      // instead of collapsing into hairline stripes.
      const cols = Math.min(p.totalCols, 4);
      wrap.style.cssText = `top:${top}px; height:${height}px;`;
      wrap.style.setProperty('--tl-cols', cols);
      wrap.style.setProperty('--tl-col', Math.min(p.col, cols - 1));
      wrap.classList.add('day-tl-block-wrap-multi');
    } else {
      wrap.style.cssText = `top:${top}px; height:${height}px;`;
    }
    wrap.innerHTML = blockHtml(p.block, label, isConflict(p.block));
    layer.appendChild(wrap);
  }

  // Free-time gaps between planned events (if 2+ events with time exist)
  const sortedEvts = [...eventIntervals].sort((a, b) => a.startMin - b.startMin);
  for (let i = 0; i + 1 < sortedEvts.length; i++) {
    const gapStart = sortedEvts[i].endMin;
    const gapEnd = sortedEvts[i + 1].startMin;
    if (gapEnd - gapStart >= 60) {
      layer.insertAdjacentHTML('beforeend', gapHtml(gapStart, gapEnd, minToPx));
    }
  }

  // Instant schedule completions ("выпил воды", reflexions) — narrow markers
  // at completed_at, height ~14px, never a full block. tracking_mode='check'
  // means the schedule never opens a timeline_block, so they need their own
  // representation here.
  const [completions, allSchedules] = await Promise.all([
    invoke('get_schedule_completions', { date }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
  ]);
  // Schedules whose completion comes from a timeline_block (timer) are
  // already drawn as proper blocks above — skip them here to avoid double.
  // What we DO want as markers: check-mode tasks + any track-mode task
  // that got marked done without a block (auto-from-Health, manual ✓).
  const blockedSchedIds = new Set(
    (blocks || [])
      .filter(b => b.source_type === 'schedule' && b.source_id != null)
      .map(b => b.source_id)
  );
  const doneChecks = (completions || []).filter(c =>
    c.completed && c.completed_at && !blockedSchedIds.has(c.schedule_id)
  );

  // Split: previous-day reflections cluster into one "📝 Рефлексия" marker
  // when there are 2+ of them — single ones stay individual.
  const reflections = doneChecks.filter(c => c.marks_previous_day);
  const regulars    = doneChecks.filter(c => !c.marks_previous_day);
  const totalReflections = (allSchedules || []).filter(s =>
    s.marks_previous_day && s.is_active !== false
  ).length;

  const renderMarker = (top, icon, label, hm, tip) => {
    const marker = document.createElement('div');
    marker.className = 'day-tl-instant';
    marker.style.cssText = `top:${top}px;`;
    marker.title = tip;
    marker.innerHTML = `<span class="day-tl-instant-ico">${icon}</span><span class="day-tl-instant-label">${escapeHtml(label)}</span><span class="day-tl-instant-time">${hm}</span>`;
    layer.appendChild(marker);
  };

  for (const c of regulars) {
    const hm = timestampToHM(c.completed_at);
    const min = parseHM(hm);
    if (min == null) continue;
    renderMarker(minToPx(min), SCH_CAT_ICONS[c.category] || '◽', c.title, hm, `${c.title} · ${hm}`);
  }

  if (reflections.length === 1) {
    const c = reflections[0];
    const hm = timestampToHM(c.completed_at);
    const min = parseHM(hm);
    if (min != null) renderMarker(minToPx(min), '📝', `Рефлексия: ${c.title}`, hm, `${c.title} · ${hm}`);
  } else if (reflections.length >= 2) {
    // Cluster: pin to the latest completed_at, summarise N/total.
    const latest = reflections.reduce((a, b) =>
      (a.completed_at > b.completed_at) ? a : b
    );
    const hm = timestampToHM(latest.completed_at);
    const min = parseHM(hm);
    const total = Math.max(totalReflections, reflections.length);
    const titles = reflections.map(r => `• ${r.title}`).join('\n');
    if (min != null) renderMarker(minToPx(min), '📝', `Рефлексия за вчера (${reflections.length}/${total})`, hm, `Рефлексия за вчера:\n${titles}`);
  }

  rootEl.appendChild(layer);
}
