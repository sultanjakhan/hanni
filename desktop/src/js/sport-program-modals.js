// ── sport-program-modals.js — Program detail modal (balance, days, run, actions) ──
import { invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { KIND_LABELS, KIND_COLORS } from './sport-program-filters.js';
import { renderBalanceBar } from './sport-program-card.js';

export async function showProgramDetail(id, onChanged) {
  const p = await invoke('get_workout_program', { id }).catch(() => null);
  if (!p) return;
  const kColor = KIND_COLORS[p.kind] || 'gray';
  const kLabel = KIND_LABELS[p.kind] || p.kind;
  const dur = p.duration_weeks ? `${p.duration_weeks} нед.` : 'бессрочно';

  const daysHtml = (p.days || []).map(d => {
    const name = d.is_rest ? 'Отдых' : (d.template_name || (d.template_id ? '— шаблон удалён —' : d.label || '—'));
    const cls = d.is_rest ? 'program-day-rest-row' : '';
    const active = p.run && p.run.current_day === d.day_index ? ' program-day-current' : '';
    return `<div class="program-day-view ${cls}${active}">
      <span class="program-day-idx">${d.day_index + 1}</span>
      <span class="program-day-name">${escapeHtml(d.label || name)}</span>
      <span class="program-day-tmpl">${d.is_rest ? '💤' : escapeHtml(name)}</span>
    </div>`;
  }).join('');

  let runHtml = '';
  if (p.run) {
    const total = p.duration_weeks ? p.duration_weeks * (p.cycle_length_days || 1) : 0;
    const pct = total ? Math.min(100, Math.round((p.run.completed_days / total) * 100)) : 0;
    runHtml = `<div class="program-run-box">
      <span>▶ Активна · день ${p.run.current_day + 1}/${p.cycle_length_days} · выполнено ${p.run.completed_days}${total ? `/${total}` : ''}</span>
      ${total ? `<div class="program-progress"><div class="program-progress-fill" style="width:${pct}%"></div></div>` : ''}
    </div>`;
  }

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:85vh;overflow-y:auto">
    <div class="modal-title">${p.active === 1 ? '● ' : ''}${p.favorite === 1 ? '★ ' : ''}${escapeHtml(p.name)}</div>
    <div class="sport-card-meta" style="margin-bottom:8px">
      <span class="badge badge-${kColor}">${kLabel}</span>
      <span class="badge badge-gray">цикл ${p.cycle_length_days}д</span>
      <span class="badge badge-gray">${dur}</span>
    </div>
    ${p.notes ? `<div style="color:var(--text-secondary);font-size:13px;margin-bottom:10px">${escapeHtml(p.notes)}</div>` : ''}
    ${runHtml}
    <div class="form-label">Баланс по группам</div>
    ${renderBalanceBar(p.muscle_volume)}
    <div class="form-label" style="margin-top:10px">Дни (${(p.days || []).length})</div>
    <div class="program-days-list">${daysHtml || '<div class="uni-empty">Нет дней</div>'}</div>
    <div class="modal-actions">
      <button class="btn-secondary" id="pd-close">Закрыть</button>
      <button class="btn-secondary" id="pd-edit">Изменить</button>
      <button class="btn-secondary" id="pd-fav">${p.favorite === 1 ? '★ Убрать' : '☆ Избранное'}</button>
      <button class="btn-danger" id="pd-delete">Удалить</button>
      ${p.run
        ? `<button class="btn-secondary" id="pd-stop">Остановить</button><button class="btn-primary" id="pd-done">Готово · день</button>`
        : `<button class="btn-primary" id="pd-start">Запустить</button>`}
    </div>
  </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  const close = () => overlay.remove();
  overlay.querySelector('#pd-close').onclick = close;

  overlay.querySelector('#pd-edit').onclick = async () => {
    const { showProgramBuilder } = await import('./sport-program-builder.js');
    close();
    showProgramBuilder(onChanged, p);
  };
  overlay.querySelector('#pd-fav').onclick = async () => { await invoke('toggle_favorite_program', { id }); close(); onChanged(); };
  overlay.querySelector('#pd-delete').onclick = async () => {
    if (!await confirmModal('Удалить программу?', 'Удалить')) return;
    await invoke('delete_workout_program', { id }); close(); onChanged();
  };
  overlay.querySelector('#pd-start')?.addEventListener('click', async () => {
    try { await invoke('start_program', { programId: id }); close(); onChanged(); }
    catch (err) { alert('Ошибка: ' + err); }
  });
  overlay.querySelector('#pd-stop')?.addEventListener('click', async () => {
    await invoke('stop_program', { runId: p.run.id }); close(); onChanged();
  });
  overlay.querySelector('#pd-done')?.addEventListener('click', async () => {
    await invoke('complete_program_day', { runId: p.run.id }); close(); onChanged();
  });
}
