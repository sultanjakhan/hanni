// ── js/routine-widget.js — Routine section of the "+" task widget ──
// Shows: chains not yet started → "Я встал / начать"; active runs → current
// available tasks. This is the daily "player" of the routine engine.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const CAT_ICONS = {
  health: '💚', sport: '🔥', hygiene: '🫧', home: '🏡',
  practice: '🎯', challenge: '⚡', growth: '🌱', work: '⚙️', other: '◽',
};

function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/// HTML for the routine section; '' when there are no chains.
export async function renderRoutineSection() {
  const date = localDate();
  const chains = await invoke('get_routine_chains').catch(() => []);
  const now = await invoke('get_routine_now', { date }).catch(() => []);
  if (!chains.length) return '';

  let rows = '';
  for (const c of chains) {
    const run = now.find(r => r.chain_id === c.id);
    if (run) {
      for (const t of run.tasks) {
        rows += `<button class="tw-item" data-rt-task="${t.id}" data-rt-run="${run.run_id}">
          <span class="tw-item-icon">${CAT_ICONS[t.category] || CAT_ICONS.other}</span>
          <span class="tw-item-title">${escapeHtml(t.title)}</span>
        </button>`;
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
      const isWake = c.trigger_type === 'sleep_end';
      const label = isWake ? `${escapeHtml(c.title)} — Я встал` : `${escapeHtml(c.title)} — начать`;
      rows += `<button class="tw-item" data-rt-chain="${c.id}">
        <span class="tw-item-icon">${isWake ? '☀️' : '▶️'}</span>
        <span class="tw-item-title">${label}</span>
      </button>`;
    }
  }
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
