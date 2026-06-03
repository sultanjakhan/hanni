// Day-grid overlay: paint completed timeline_blocks as absolutely-positioned
// rectangles spanning their real start..end time. Also marks free gaps and
// highlights blocks that conflict with planned events.

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { showTimelinePopover, showPagerPopover } from './calendar-event-popover.js';

// Hour height defaults to 48px; caller passes the current zoom value.
const DEFAULT_HOUR_PX = 48;

const SOURCE_COLOR = { event: '#60a5fa', schedule: '#c084fc', note: '#4ade80', manual: '#94a3b8' };
const SOURCE_BG    = { event: '#dbeafe', schedule: '#f3e8ff', note: '#dcfce7', manual: '#f1f5f9' };
const SOURCE_NAME  = { event: 'Событие', schedule: 'Расписание', note: 'Заметка', manual: 'Вручную' };
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

  // Time context for "free" gaps and marker spacing. Past time isn't "free".
  const now = new Date();
  const pad2 = (n) => String(n).padStart(2, '0');
  const todayStr = `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
  const dayIsPast = date < todayStr;
  const nowMin = (date === todayStr) ? now.getHours() * 60 + now.getMinutes() : null;
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

  // Event pixel boxes (+ the event obj) so a completion that lands on a planned
  // event folds into it as a "+N" badge and a click-through pager, instead of
  // being drawn as its own marker competing for the same row.
  const EV_SOURCE = { manual: 'Вручную', apple: 'Apple Calendar', auto_health: 'Apple Health', google: 'Google Calendar' };
  const toHM = (m) => `${String(Math.floor(m / 60) % 24).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
  const MARKER_PX = 16;
  const evBoxes = (plannedEvents || [])
    .filter(e => e.time && e.id != null)
    .map(e => {
      const s = parseHM(e.time); if (s == null) return null;
      const top = minToPx(s);
      return { id: e.id, ev: e, top, bottom: top + Math.max(minToPx(e.duration_minutes || 60), 22) };
    })
    .filter(Boolean);
  const groups = new Map();  // event id -> [{ title, time, isReflection }]
  // Fold a completion at `min` into the event it overlaps (if any). Returns true
  // when folded — caller then skips drawing it as a standalone marker.
  const foldInto = (min, item) => {
    const top = minToPx(min);
    const hit = evBoxes.find(b => top < b.bottom && top + MARKER_PX > b.top);
    if (!hit) return false;
    if (!groups.has(hit.id)) groups.set(hit.id, []);
    groups.get(hit.id).push(item);
    return true;
  };
  const eventPagerItem = (e) => {
    const s = parseHM(e.time);
    const dur = e.duration_minutes || 0;
    const rows = [];
    if (dur) rows.push({ text: `⏱ ${e.source === 'auto_health' ? 'Факт' : 'Длительность'}: ${fmtDur(dur)}` });
    rows.push({ text: EV_SOURCE[e.source] || 'Вручную', muted: true });
    rows.push(e.completed ? { text: '✓ Выполнено', done: true } : { text: '○ Не отмечено', muted: true });
    return { title: e.title || 'Событие', subtitle: s != null ? `${e.time} – ${toHM(s + dur)}` : (e.time || ''), rows };
  };

  // Container fills the full day height of .day-timeline (24 * hourPx).
  const layer = document.createElement('div');
  layer.className = 'day-tl-layer';
  layer.style.height = `${24 * hourPx}px`;
  // Thin completion markers live in a separate layer above event blocks (z:4)
  // so a tick at the same time as an event stays visible and clickable.
  const markerLayer = document.createElement('div');
  markerLayer.className = 'day-tl-marker-layer';
  markerLayer.style.height = `${24 * hourPx}px`;

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
    const conflict = isConflict(p.block);
    wrap.innerHTML = blockHtml(p.block, label, conflict);
    wrap.addEventListener('click', (ev) => {
      ev.stopPropagation();
      const start = p.block.start_time.slice(0, 5);
      const end = p.block.end_time.slice(0, 5);
      const rows = [
        { text: `🕐 ${start} – ${end}` },
        { text: `⏱ Факт: ${fmtDur(p.block.duration_minutes || 0)}` },
        { text: SOURCE_NAME[p.block.source_type] || p.block.type_name || 'Активность', muted: true },
        { text: '✓ Выполнено', done: true },
      ];
      if (conflict) rows.push({ text: '⚠️ конфликт с встречей', muted: true });
      showTimelinePopover(ev.clientX, ev.clientY, { title: label || p.block.type_name || 'Активность', rows });
    });
    layer.appendChild(wrap);
  }

  // Free-time gaps between planned events (if 2+ events with time exist)
  const sortedEvts = [...eventIntervals].sort((a, b) => a.startMin - b.startMin);
  for (let i = 0; i + 1 < sortedEvts.length; i++) {
    let gapStart = sortedEvts[i].endMin;
    const gapEnd = sortedEvts[i + 1].startMin;
    // "Свободно" means time still free to plan — skip gaps already in the past.
    if (dayIsPast) continue;
    if (nowMin != null) {
      if (gapEnd <= nowMin) continue;            // gap fully elapsed
      if (gapStart < nowMin) gapStart = nowMin;  // keep only the still-free part
    }
    if (gapEnd - gapStart >= 60) {
      layer.insertAdjacentHTML('beforeend', gapHtml(gapStart, gapEnd, minToPx));
    }
  }

  // Instant schedule completions ("выпил воды", reflexions) — narrow markers
  // at completed_at, height ~14px, never a full block. tracking_mode='check'
  // means the schedule never opens a timeline_block, so they need their own
  // representation here.
  const prevD = new Date(date + 'T12:00:00'); prevD.setDate(prevD.getDate() - 1);
  const prevDate = `${prevD.getFullYear()}-${pad2(prevD.getMonth() + 1)}-${pad2(prevD.getDate())}`;
  const [completions, prevCompletions, allSchedules] = await Promise.all([
    invoke('get_schedule_completions', { date }).catch(() => []),
    invoke('get_schedule_completions', { date: prevDate }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
  ]);
  // Schedules whose completion comes from a timeline_block (timer) are
  // already drawn as proper blocks above — skip them here to avoid double.
  const blockedSchedIds = new Set(
    (blocks || [])
      .filter(b => b.source_type === 'schedule' && b.source_id != null)
      .map(b => b.source_id)
  );
  // Show what was actually DONE on this calendar day, keyed by completed_at:
  // regular ticks live under `date`; "marks previous day" reflections done today
  // live under yesterday's date. Merge both, keep only ones completed on `date`,
  // dedup by schedule (a re-tick can leave a row under each date).
  const byId = new Map();
  for (const c of [...(completions || []), ...(prevCompletions || [])]) {
    if (!c.completed || !c.completed_at || !c.completed_at.startsWith(date)) continue;
    if (blockedSchedIds.has(c.schedule_id)) continue;
    const prev = byId.get(c.schedule_id);
    if (!prev || c.completed_at > prev.completed_at) byId.set(c.schedule_id, c);
  }
  const doneChecks = [...byId.values()];

  const reflections = doneChecks.filter(c => c.marks_previous_day);
  const regulars    = doneChecks.filter(c => !c.marks_previous_day);
  const reflSchedules = (allSchedules || []).filter(s => s.marks_previous_day && s.is_active !== false);

  // Each completion-group is drawn as a clear box (like an event): the first
  // task's name + a "+N" badge for the rest; clicking pages through them with
  // arrows. Event-coincident ticks fold into the event instead (foldInto).
  const markerGapMin = Math.max(10, Math.ceil(18 / (hourPx / 60)));
  const pendingGroups = [];
  const buildTaskGroups = (list) => {
    const pts = list
      .map(c => ({ c, min: parseHM(timestampToHM(c.completed_at)) }))
      .filter(p => p.min != null)
      .filter(p => !foldInto(p.min, { title: p.c.title, time: timestampToHM(p.c.completed_at), scheduleId: p.c.schedule_id, undoDate: date }))
      .sort((a, b) => a.min - b.min);
    let grp = [];
    const flush = () => {
      if (!grp.length) return;
      const first = grp[0];
      pendingGroups.push({
        top: minToPx(first.min),
        icon: SCH_CAT_ICONS[first.c.category] || '◽',
        title: first.c.title,
        badge: grp.length > 1 ? `+${grp.length - 1}` : '',
        hm: timestampToHM(first.c.completed_at),
        items: grp.map(p => ({
          title: p.c.title, subtitle: timestampToHM(p.c.completed_at), accent: '#c084fc',
          rows: [{ text: '✓ Выполнено', done: true }],
          toggle: { scheduleId: p.c.schedule_id, date, done: true },
        })),
      });
      grp = [];
    };
    for (const p of pts) {
      if (grp.length && p.min - grp[grp.length - 1].min > markerGapMin) flush();
      grp.push(p);
    }
    flush();
  };
  buildTaskGroups(regulars);

  // Reflections → one box for the "yesterday" set. The badge shows done/total
  // (e.g. 12/29); the pager browses the completed ones (the rest are managed in
  // the schedule list, not on the "what I did" timeline).
  const doneRefl = reflections
    .map(c => ({ c, min: parseHM(timestampToHM(c.completed_at)) }))
    .filter(p => p.min != null)
    .filter(p => !foldInto(p.min, { title: p.c.title, time: timestampToHM(p.c.completed_at), isReflection: true, scheduleId: p.c.schedule_id, undoDate: prevDate }))
    .sort((a, b) => a.min - b.min);
  if (doneRefl.length) {
    const doneIds = new Set(doneRefl.map(p => p.c.schedule_id));
    const items = [
      ...doneRefl.map(p => ({
        title: p.c.title, subtitle: timestampToHM(p.c.completed_at), accent: '#f59e0b',
        rows: [{ text: '📝 Рефлексия за вчера', muted: true }, { text: '✓ Выполнено', done: true }],
        toggle: { scheduleId: p.c.schedule_id, date: prevDate, done: true },
      })),
      ...reflSchedules.filter(s => !doneIds.has(s.id)).map(s => ({
        title: s.title, subtitle: 'не отмечено', accent: '#f59e0b',
        rows: [{ text: '📝 Рефлексия за вчера', muted: true }, { text: '○ Не отмечено', muted: true }],
        toggle: { scheduleId: s.id, date: prevDate, done: false },
      })),
    ];
    pendingGroups.push({
      top: minToPx(doneRefl[0].min),
      icon: '📝',
      title: 'Рефлексия за вчера',
      badge: `${doneRefl.length}/${Math.max(reflSchedules.length, doneRefl.length)}`,
      hm: timestampToHM(doneRefl[0].c.completed_at),
      items,
      reflection: true,
    });
  }

  // Place group boxes top-down, nudging overlaps downward so none sits on another.
  pendingGroups.sort((a, b) => a.top - b.top);
  let lastBottom = -Infinity;
  for (const g of pendingGroups) {
    const top = Math.max(g.top, lastBottom + 3);
    lastBottom = top + 28;
    const box = document.createElement('div');
    box.className = 'day-tl-group' + (g.reflection ? ' day-tl-group-refl' : '');
    box.style.cssText = `top:${top}px;`;
    box.title = g.items.map(it => `• ${it.title}`).join('\n');
    box.innerHTML = `<span class="day-tl-group-ico">${g.icon}</span><span class="day-tl-group-title">${escapeHtml(g.title)}</span>${g.badge ? `<span class="day-tl-group-count">${g.badge}</span>` : ''}<span class="day-tl-group-time">${g.hm}</span>`;
    box.addEventListener('click', (ev) => { ev.stopPropagation(); showPagerPopover(g.items, ev.clientX, ev.clientY); });
    markerLayer.appendChild(box);
  }

  rootEl.appendChild(layer);
  rootEl.appendChild(markerLayer);

  // Fold overlapping completions into their event: a "✓ N" badge on the block
  // and a click-through pager (page 1 = event, then each completed task).
  for (const [id, items] of groups) {
    const box = evBoxes.find(b => b.id === id);
    const blk = rootEl.querySelector(`.day-event-block[data-evt-pop="${id}"]`);
    if (!box || !blk) continue;
    blk.__evtGroup = [{ ...eventPagerItem(box.ev), accent: box.ev.color }, ...items.map(it => ({
      title: it.title,
      subtitle: it.time,
      accent: it.isReflection ? '#f59e0b' : '#c084fc',
      rows: [it.isReflection ? { text: '📝 Рефлексия за вчера', muted: true } : null, { text: '✓ Выполнено', done: true }].filter(Boolean),
      toggle: { scheduleId: it.scheduleId, date: it.undoDate, done: true },
    }))];
    const badge = document.createElement('span');
    badge.className = 'day-event-count';
    badge.textContent = `✓ ${items.length}`;
    badge.title = `${items.length} выполненных задач рядом — нажмите, чтобы листать`;
    (blk.querySelector('.day-event-block-head') || blk).appendChild(badge);
  }
}
