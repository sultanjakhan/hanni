// ── js/routine-widget.js — Routine section of the "+" task widget ──
// Shows: chains not yet started → "Я встал / начать"; active runs → current
// available tasks. This is the daily "player" of the routine engine.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { timeToMin } from './task-picker-sort.js';
import { isDanKoePractice } from './dankoe-quick-modal.js';

const CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽',
};

// Used by wireRoutineSection to pass today's date to backend commands.
function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/// HTML for the routine section; '' when there are no chains. `now` is the
/// get_routine_now payload; `recommendedId` is the active task to highlight blue;
/// `chainRecId` is the chain whose start button to highlight as "сейчас".
const MEAL3 = ['Завтрак', 'Обед', 'Ужин'];
const parseTimes = (tt) => String(tt || '').split(',').map(s => s.trim()).filter(Boolean);

export async function renderRoutineSection(chains = [], now = [], recommendedId = null, chainRecId = null, completedChainIds = [], dueChainIds = null) {
  if (!chains.length) return '';

  // completedChainIds is now [{chain_id, slot}] — key by "chainId:slot".
  const completedSet = new Set((completedChainIds || []).map(x => `${x.chain_id}:${x.slot || ''}`));
  const nowMin = new Date().getHours() * 60 + new Date().getMinutes();

  // Active-run steps (✓/✗ + locked) and a cancel button.
  const renderRun = (run, label) => {
    let h = '';
    for (const t of run.tasks) {
      const cur = t.id === recommendedId ? ' tw-rt-step--now' : '';
      // Control per step kind:
      //   • Dan Koe practice  → ▶ opens the journaling modal (data-rt-dankoe)
      //   • timer (track)     → ▶ starts its schedule's timer (data-rt-start)
      //   • reflection (marks_previous_day) → ✓/✗ retrospective yes/no
      //   • plain instant check (Встал, Витамины…) → single ✓, no skip
      // ✗ (skip) shows for everything EXCEPT plain instant checks — those are a
      // one-tap "done", not a +/- pair.
      const isReflection = !!t.marks_previous_day;
      const isCheck = t.tracking_mode === 'check' || isReflection;
      const isDanKoe = isDanKoePractice(t.title);
      const isTrack = t.source_type === 'schedule' && t.source_id != null && !isCheck && !isDanKoe;
      const isInstantCheck = isCheck && !isReflection;
      let positive;
      if (isDanKoe) {
        positive = `<span class="tw-rt-start" data-rt-dankoe="${t.id}" data-rt-run="${run.run_id}" data-rt-title="${escapeHtml(t.title)}" data-rt-sched="${escapeHtml(String(t.source_id))}" title="Открыть">▶</span>`;
      } else if (isTrack) {
        positive = `<span class="tw-rt-start" data-rt-start="${t.id}" data-rt-run="${run.run_id}" data-rt-sched="${escapeHtml(String(t.source_id))}" title="Старт">▶</span>`;
      } else {
        positive = `<span class="tw-check" data-rt-task="${t.id}" data-rt-run="${run.run_id}" title="Сделал">✓</span>`;
      }
      const skip = isInstantCheck ? ''
        : `<span class="tw-skip" data-rt-skip="${t.id}" data-rt-run="${run.run_id}" title="Не выполнено">✗</span>`;
      h += `<div class="tw-item tw-rt-step${cur}">
        <span class="tw-item-icon">${CAT_ICONS[t.category] || CAT_ICONS.other}</span>
        <span class="tw-item-title">${escapeHtml(t.title)}</span>
        ${positive}
        ${skip}
      </div>`;
    }
    for (const t of (run.locked || [])) {
      h += `<button class="tw-item tw-rt-locked" data-rt-unlock="${t.id}" data-rt-run="${run.run_id}">
        <span class="tw-item-icon">🔒</span>
        <span class="tw-item-title">Открыть «${escapeHtml(t.title)}»</span>
      </button>`;
    }
    h += `<button class="tw-item tw-rt-cancel" data-rt-cancel="${run.run_id}">
      <span class="tw-item-icon">✕</span>
      <span class="tw-item-title">Отменить «${escapeHtml(label)}»</span>
    </button>`;
    return h;
  };
  const renderLaunch = (c, slot, label, isWake, isRec) => `
    <button class="tw-item ${isRec ? 'tw-item--recommended' : ''}" data-rt-chain="${c.id}" data-rt-slot="${escapeHtml(slot)}">
      <span class="tw-item-icon">${isWake ? '☀️' : '▶️'}</span>
      <span class="tw-item-title">${escapeHtml(label)}</span>
      ${isRec ? '<span class="tw-now">сейчас</span>' : ''}
    </button>`;

  let rows = '';
  for (const c of chains) {
    const times = c.trigger_type === 'time' ? parseTimes(c.trigger_time) : [];
    // Multi-slot chain (e.g. meals): a separate launch/run per time-of-day.
    if (times.length > 1) {
      const runsBySlot = {};
      now.forEach(r => { if (r.chain_id === c.id) runsBySlot[r.slot] = r; });
      times.forEach((t, i) => {
        // Meal chains surface as their slot name (Завтрак/Обед/Ужин) — the chain
        // itself ("Еда") never appears as a launch in the player.
        const label = times.length === 3 ? MEAL3[i] : t;
        const run = runsBySlot[t];
        if (run) {
          if (!run.tasks.length && !(run.locked || []).length) return;
          rows += renderRun(run, label);
        } else if (completedSet.has(`${c.id}:${t}`)) {
          // this meal already done today
        } else if (nowMin >= timeToMin(t)) {
          rows += renderLaunch(c, t, `${label} — начать`, false, false);
        }
      });
      continue;
    }
    // Normal chain — one run per day (slot='').
    if (completedSet.has(`${c.id}:`) && !now.find(r => r.chain_id === c.id)) continue;
    const run = now.find(r => r.chain_id === c.id);
    // Run with nothing left to do (last step just closed → completing): let the
    // whole chain disappear instead of leaving a dead-end lone "Отменить".
    if (run && !run.tasks.length && !(run.locked || []).length) continue;
    if (run) {
      rows += renderRun(run, c.title);
    } else {
      // Time/day-gate: hide an unstarted chain when its first step isn't "к месту" now.
      if (dueChainIds && !dueChainIds.has(c.id)) continue;
      const isWake = c.trigger_type === 'sleep_end';
      const label = isWake ? `${c.title} — Я встал` : `${c.title} — начать`;
      rows += renderLaunch(c, '', label, isWake, c.id === chainRecId);
    }
  }
  if (!rows) return '';
  return `<div class="tw-group-header">Рутина</div>${rows}`;
}

/// Wire clicks for the routine section. `onChange` re-renders the dropdown;
/// `onStarted` runs after a ▶ timer start (closes the picker — defaults to onChange);
/// `onDanKoe(title, scheduleId)` opens the Dan Koe journaling modal for ▶ practice steps.
export function wireRoutineSection(panel, onChange, onStarted = onChange, onDanKoe = null) {
  panel.querySelectorAll('[data-rt-dankoe]').forEach(btn => {
    btn.addEventListener('click', () => {
      onDanKoe?.(btn.dataset.rtTitle, String(btn.dataset.rtSched));
    });
  });
  panel.querySelectorAll('[data-rt-start]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('start_task_block', {
        sourceType: 'schedule', sourceId: String(btn.dataset.rtSched),
      }).catch(() => {});
      onStarted();
    });
  });
  panel.querySelectorAll('[data-rt-chain]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('start_routine_run', {
        chainId: parseInt(btn.dataset.rtChain), date: localDate(),
        slot: btn.dataset.rtSlot || '',
      }).catch(() => {});
      onChange();
    });
  });
  panel.querySelectorAll('[data-rt-task]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('set_routine_node_status', {
        runId: parseInt(btn.dataset.rtRun),
        nodeId: parseInt(btn.dataset.rtTask),
        state: 'done',
      }).catch(() => {});
      onChange();
    });
  });
  panel.querySelectorAll('[data-rt-skip]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('set_routine_node_status', {
        runId: parseInt(btn.dataset.rtRun),
        nodeId: parseInt(btn.dataset.rtSkip),
        state: 'skipped',
      }).catch(() => {});
      onChange();
    });
  });
  panel.querySelectorAll('[data-rt-unlock]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('set_routine_node_status', {
        runId: parseInt(btn.dataset.rtRun),
        nodeId: parseInt(btn.dataset.rtUnlock),
        state: 'unlocked',
      }).catch(() => {});
      onChange();
    });
  });
  panel.querySelectorAll('[data-rt-cancel]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('delete_routine_run', { runId: parseInt(btn.dataset.rtCancel) }).catch(() => {});
      onChange();
    });
  });
}
