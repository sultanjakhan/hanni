// ── sport-programs.js — Workout programs pane (browse + active-program "today" strip) ──
import { invoke } from './state.js';
import { chips, escapeHtml } from './utils.js';
import { renderProgramCard } from './sport-program-card.js';
import { PROGRAM_KINDS, matchKind, matchSearch } from './sport-program-filters.js';

export async function renderProgramsPane(el) {
  const F = { kind: 'all', fav: false, q: '' };
  let all = [], panelOpen = false, built = false;

  async function loadData() {
    all = await invoke('get_workout_programs', { search: null }).catch(() => []);
  }
  function getFiltered() {
    return all.filter(p => matchKind(p, F.kind) && matchSearch(p, F.q) && (!F.fav || p.favorite === 1));
  }

  function buildShell() {
    el.innerHTML = `<div class="recipe-pane">
      <div class="program-today" style="display:none"></div>
      <div class="recipe-filter-bar">
        <button class="rf-toggle" title="Фильтр"><svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M1.5 2h13L9.5 8.5V13l-3 1.5V8.5z"/></svg><span class="rf-badge" style="display:none"></span></button>
        <input class="recipe-search" type="text" placeholder="Поиск программ...">
        <button class="btn-primary recipe-add-btn">+ Программа</button>
      </div>
      <div class="rf-panel" style="display:none"></div>
      <div class="recipe-grid"></div></div>`;
    el.querySelector('.rf-toggle').onclick = () => { panelOpen = !panelOpen; el.querySelector('.rf-panel').style.display = panelOpen ? '' : 'none'; };
    el.querySelector('.recipe-search').oninput = (e) => { F.q = e.target.value.trim().toLowerCase(); updateGrid(); };
    el.querySelector('.recipe-add-btn').onclick = async () => {
      const { showProgramBuilder } = await import('./sport-program-builder.js');
      showProgramBuilder(fullReload);
    };
  }

  function updatePanel() {
    const panel = el.querySelector('.rf-panel');
    panel.innerHTML = `
      <div class="rf-section"><span class="rf-title">Тип</span>${chips(PROGRAM_KINDS, F.kind, 'kind')}</div>
      <div class="rf-section"><button class="rf-chip${F.fav ? ' active' : ''}" data-group="fav" data-val="toggle">★ Избранное</button></div>`;
    panel.querySelectorAll('.rf-chip').forEach(btn => btn.onclick = () => {
      const g = btn.dataset.group, v = btn.dataset.val;
      if (g === 'fav') F.fav = !F.fav; else F[g] = v;
      updatePanel(); updateGrid(); updateBadge();
    });
    updateBadge();
  }

  function updateBadge() {
    const ac = [F.kind !== 'all', F.fav].filter(Boolean).length;
    const badge = el.querySelector('.rf-badge'), toggle = el.querySelector('.rf-toggle');
    badge.textContent = ac; badge.style.display = ac ? '' : 'none';
    toggle.classList.toggle('rf-active', ac > 0);
  }

  async function renderToday() {
    const strip = el.querySelector('.program-today');
    if (!strip) return;
    const today = await invoke('get_today_program_workout').catch(() => null);
    if (!today) { strip.style.display = 'none'; strip.innerHTML = ''; return; }
    strip.style.display = '';
    const labels = (today.days || []).map(d => d.is_rest ? 'Отдых' : (d.template_name || d.label || '—')).join(', ') || 'Отдых';
    strip.innerHTML = `
      <div class="program-today-info">
        <span class="program-today-name">▶ ${escapeHtml(today.program_name)}</span>
        <span class="program-today-day">День ${today.current_day + 1}/${today.cycle_length_days} · ${escapeHtml(labels)}</span>
      </div>
      <button class="btn-primary program-today-done">Готово</button>`;
    strip.querySelector('.program-today-done').onclick = async () => {
      await invoke('complete_program_day', { runId: today.run_id }).catch(() => {});
      await fullReload();
    };
  }

  function updateGrid() {
    const list = getFiltered(), grid = el.querySelector('.recipe-grid');
    grid.innerHTML = '';
    if (!list.length) { grid.innerHTML = '<div class="uni-empty">Нет программ. Создайте первую!</div>'; return; }
    for (const p of list) {
      const card = renderProgramCard(p);
      card.onclick = async () => {
        const { showProgramDetail } = await import('./sport-program-modals.js');
        showProgramDetail(p.id, fullReload);
      };
      grid.appendChild(card);
    }
  }

  async function fullReload() {
    await loadData();
    if (!built) { buildShell(); built = true; }
    updatePanel(); updateGrid(); await renderToday();
  }
  await fullReload();
}
