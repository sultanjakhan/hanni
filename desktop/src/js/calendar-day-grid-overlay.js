// Day-grid overlay: paint completed timeline_blocks as absolutely-positioned
// rectangles spanning their real start..end time. Also marks free gaps and
// highlights blocks that conflict with planned events.

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

// Hour height defaults to 48px; caller passes the current zoom value.
const DEFAULT_HOUR_PX = 48;

const SOURCE_COLOR = { event: '#60a5fa', schedule: '#c084fc', note: '#4ade80', manual: '#94a3b8' };
const SOURCE_BG    = { event: '#dbeafe', schedule: '#f3e8ff', note: '#dcfce7', manual: '#f1f5f9' };

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

function gapHtml(startMin, endMin) {
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

  // Decorate with parsed minutes + sort by start_time for cluster building
  const positioned = completed
    .map(b => ({ block: b, startMin: parseHM(b.start_time), endMin: parseHM(b.end_time) }))
    .filter(x => x.startMin != null && x.endMin != null && x.endMin > x.startMin)
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
      // Split available width into N columns. CSS handles the math via custom props.
      wrap.style.cssText = `top:${top}px; height:${height}px;`;
      wrap.style.setProperty('--tl-cols', p.totalCols);
      wrap.style.setProperty('--tl-col', p.col);
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
      layer.insertAdjacentHTML('beforeend', gapHtml(gapStart, gapEnd));
    }
  }

  rootEl.appendChild(layer);
}
