// Shared recommendation data for the floating task picker and Calendar "Now" card.
import { invoke } from './state.js';
import { loadCategoryWeights } from './effective-priority.js';
import {
  nowMinutes,
  pickRecommendedTaskId,
  pickStartChainId,
  rankTasks,
  timeToMin,
} from './task-picker-sort.js';

export function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function scheduleDueNow(schedule, dayOfWeek, currentMinute) {
  if (!schedule || !schedule.is_active) return false;
  let dayMatches = false;
  if (schedule.frequency === 'daily') dayMatches = true;
  if (schedule.frequency === 'weekly' || schedule.frequency === 'custom') {
    dayMatches = String(schedule.frequency_days || '').split(',').map(Number).includes(dayOfWeek);
  }
  if (!dayMatches) return false;
  const visibleFrom = timeToMin(schedule.visible_from);
  return visibleFrom == null || currentMinute >= visibleFrom;
}

function chainDueNow(chain, schedulesById, dayOfWeek, currentMinute) {
  if (chain.trigger_type === 'time' && chain.trigger_time) {
    const times = String(chain.trigger_time).split(',').map(v => timeToMin(v.trim())).filter(v => v != null);
    return times.length === 0 || currentMinute >= Math.min(...times);
  }
  const start = (chain.nodes || []).find(node => node.is_start);
  const incoming = {};
  (chain.edges || []).forEach(edge => {
    (incoming[edge.to_node_id] = incoming[edge.to_node_id] || []).push(edge.from_node_id);
  });
  const entries = (chain.nodes || []).filter(node => !node.is_start &&
    ((incoming[node.id] || []).length === 0 ||
      (incoming[node.id] || []).every(fromId => fromId === start?.id)));
  if (!entries.length) return true;
  return entries.some(node => !node.source_id ||
    scheduleDueNow(schedulesById[String(node.source_id)], dayOfWeek, currentMinute));
}

function regularReason(item, currentMinute) {
  if (item._overdue) return 'Недавно просрочено';
  const planned = timeToMin(item.planned_time);
  if (planned != null && planned >= currentMinute && planned - currentMinute <= 60) {
    return 'Запланировано на ближайший час';
  }
  if ((item.priority || 0) > 0) return 'Выше по важности';
  return 'Первое доступное действие';
}

function regularCandidates(startable, weights, pins, avgDur, currentMinute) {
  const ranked = rankTasks(startable, { nowMin: currentMinute, weights, pins });
  const overdue = ranked.filter(item => !item._pinned && item._overdue);
  const rest = ranked.filter(item => !item._pinned && !item._overdue);
  const ordered = [
    ...overdue,
    ...['event', 'schedule', 'note'].flatMap(type => rest.filter(item => item.source_type === type)),
  ];
  return ordered.map(item => ({
    ...item,
    kind: 'task',
    key: `task:${item.source_type}:${item.source_id}`,
    reason: regularReason(item, currentMinute),
    durationMinutes: avgDur[`${item.source_type}:${item.source_id}`] || item.duration_minutes || null,
  }));
}

function activeRoutineRecommendation(runs, recommendedId) {
  if (recommendedId == null) return null;
  for (const run of runs || []) {
    const item = (run.tasks || []).find(task => task.id === recommendedId);
    if (item) return {
      ...item,
      kind: 'routine-task',
      key: `routine-task:${run.run_id}:${item.id}`,
      runId: run.run_id,
      nodeId: item.id,
      reason: 'Следующий шаг активной рутины',
      durationMinutes: item.duration_minutes || null,
    };
  }
  return null;
}

function chainRecommendation(chains, chainId) {
  const chain = (chains || []).find(item => item.id === chainId);
  if (!chain) return null;
  return {
    kind: 'routine-chain',
    key: `routine-chain:${chain.id}`,
    chainId: chain.id,
    slot: '',
    title: chain.trigger_type === 'sleep_end' ? `${chain.title} — Я встал` : `${chain.title} — начать`,
    reason: 'Рутина доступна сейчас',
    durationMinutes: null,
  };
}

export async function loadTaskRecommendationData() {
  const date = localDate();
  const [planned, weights, pins, avgRows, chains, runs, completedChains, schedules] = await Promise.all([
    invoke('get_today_planned', { date }).catch(() => []),
    loadCategoryWeights(invoke),
    invoke('get_task_pins').catch(() => []),
    invoke('get_task_avg_durations').catch(() => []),
    invoke('get_routine_chains').catch(() => []),
    invoke('get_routine_now', { date }).catch(() => []),
    invoke('get_completed_routine_chains', { date }).catch(() => []),
    invoke('get_schedules', { category: null }).catch(() => []),
  ]);
  const currentMinute = nowMinutes();
  const startable = planned.filter(item => {
    const visibleFrom = timeToMin(item.visible_from);
    return !item.completed && !item.is_active && item.status_extra !== 'done' &&
      item.status_extra !== 'skipped' && (visibleFrom == null || currentMinute >= visibleFrom);
  });
  const avgDur = Object.fromEntries(avgRows.map(row => [`${row.source_type}:${row.source_id}`, row.avg_minutes]));
  const schedulesById = Object.fromEntries(schedules.map(item => [String(item.id), item]));
  const dayOfWeek = new Date().getDay() || 7;
  const dueChainIds = new Set(chains.filter(chain =>
    chainDueNow(chain, schedulesById, dayOfWeek, currentMinute)).map(chain => chain.id));
  const routineRecId = pickRecommendedTaskId(runs);
  const recommendableChains = chains.filter(chain => {
    const times = String(chain.trigger_time || '').split(',').filter(Boolean);
    return dueChainIds.has(chain.id) && times.length <= 1;
  });
  const chainRecId = routineRecId == null
    ? pickStartChainId(recommendableChains, runs, completedChains.map(item => item.chain_id))
    : null;
  const regular = regularCandidates(startable, weights, pins, avgDur, currentMinute);
  const primary = activeRoutineRecommendation(runs, routineRecId) || chainRecommendation(chains, chainRecId);

  return {
    startable, weights, pins, avgDur, chains, runs, completedChains, dueChainIds,
    routineRecId, chainRecId,
    recommendations: primary ? [primary, ...regular] : regular,
  };
}

export async function loadActiveTaskBlock() {
  const blocks = await invoke('get_timeline_blocks', { date: localDate() }).catch(() => []);
  return blocks.find(block => block.is_active) || null;
}
