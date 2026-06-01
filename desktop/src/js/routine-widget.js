// ── js/routine-widget.js — Routine section of the "+" task widget ──
// Shows: chains not yet started → "Я встал / начать"; active runs → current
// available tasks. This is the daily "player" of the routine engine.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

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
export async function renderRoutineSection(chains = [], now = [], recommendedId = null, chainRecId = null, completedChainIds = [], dueChainIds = null) {
  if (!chains.length) return '';

  const completedSet = new Set(completedChainIds);
  let rows = '';
  for (const c of chains) {
    if (completedSet.has(c.id) && !now.find(r => r.chain_id === c.id)) continue;
    const run = now.find(r => r.chain_id === c.id);
    // Run with nothing left to do (last step just closed → completing): let the
    // whole chain disappear instead of leaving a dead-end lone "Отменить".
    if (run && !run.tasks.length && !(run.locked || []).length) continue;
    if (run) {
      for (const t of run.tasks) {
        // Explicit ✓ (сделал) / ✗ (не выполнено) pair — same affordance as the
        // schedule rows in Список/picker, so the choice is obvious instead of
        // "tap the title = done, tiny ✕ = not done". The current step gets a
        // subtle left-accent (not a full fill) so the boxes stay readable.
        const now = t.id === recommendedId ? ' tw-rt-step--now' : '';
        rows += `<div class="tw-item tw-rt-step${now}">
          <span class="tw-item-icon">${CAT_ICONS[t.category] || CAT_ICONS.other}</span>
          <span class="tw-item-title">${escapeHtml(t.title)}</span>
          <span class="tw-check" data-rt-task="${t.id}" data-rt-run="${run.run_id}" title="Сделал">✓</span>
          <span class="tw-skip" data-rt-skip="${t.id}" data-rt-run="${run.run_id}" title="Не выполнено">✗</span>
        </div>`;
      }
      for (const t of (run.locked || [])) {
        rows += `<button class="tw-item tw-rt-locked" data-rt-unlock="${t.id}" data-rt-run="${run.run_id}">
          <span class="tw-item-icon">🔒</span>
          <span class="tw-item-title">Открыть «${escapeHtml(t.title)}»</span>
        </button>`;
      }
      rows += `<button class="tw-item tw-rt-cancel" data-rt-cancel="${run.run_id}">
        <span class="tw-item-icon">✕</span>
        <span class="tw-item-title">Отменить «${escapeHtml(c.title)}»</span>
      </button>`;
    } else {
      // Time/day-gate: hide an unstarted chain when its first step isn't "к месту" now.
      if (dueChainIds && !dueChainIds.has(c.id)) continue;
      const isWake = c.trigger_type === 'sleep_end';
      const label = isWake ? `${escapeHtml(c.title)} — Я встал` : `${escapeHtml(c.title)} — начать`;
      const isRec = c.id === chainRecId;
      rows += `<button class="tw-item ${isRec ? 'tw-item--recommended' : ''}" data-rt-chain="${c.id}">
        <span class="tw-item-icon">${isWake ? '☀️' : '▶️'}</span>
        <span class="tw-item-title">${label}</span>
        ${isRec ? '<span class="tw-now">сейчас</span>' : ''}
      </button>`;
    }
  }
  if (!rows) return '';
  return `<div class="tw-group-header">Рутина</div>${rows}`;
}

/// Wire clicks for the routine section. `onChange` re-renders the dropdown.
export function wireRoutineSection(panel, onChange) {
  panel.querySelectorAll('[data-rt-chain]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke('start_routine_run', {
        chainId: parseInt(btn.dataset.rtChain), date: localDate(),
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
