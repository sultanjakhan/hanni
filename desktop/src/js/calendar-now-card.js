// Persistent Calendar surface for one concrete next action.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { isDanKoePractice, openDanKoeModal } from './dankoe-quick-modal.js';
import { loadActiveTaskBlock, loadTaskRecommendationData, localDate } from './task-recommendation-data.js';

let calendarRoot = null;
let observer = null;
let refreshTimer = null;
let renderVersion = 0;
const dismissed = new Set();

function formatMinutes(value) {
  if (!value) return '';
  const minutes = Math.round(value);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return hours ? `${hours} ч${rest ? ` ${rest} мин` : ''}` : `${rest} мин`;
}

function openTaskPicker() {
  window.dispatchEvent(new Event('hanni:task-control-open'));
}

function notifyChanged() {
  dismissed.clear();
  window.dispatchEvent(new Event('task-state-changed'));
  window.dispatchEvent(new CustomEvent('hanni:calendar-refresh'));
}

function actionLabel(item) {
  if (isDanKoePractice(item.title)) return 'Открыть';
  const isCheck = item.tracking_mode === 'check' || item.marks_previous_day;
  return item.kind === 'routine-task' && isCheck ? 'Готово' : 'Начать';
}

async function runRecommendation(item) {
  if (isDanKoePractice(item.title) && item.source_id != null) {
    await openDanKoeModal(item.title, String(item.source_id), notifyChanged);
    return;
  }
  if (item.kind === 'routine-chain') {
    await invoke('start_routine_run', { chainId: item.chainId, date: localDate(), slot: item.slot || '' });
  } else if (item.kind === 'routine-task') {
    const isCheck = item.tracking_mode === 'check' || item.marks_previous_day;
    if (item.source_type === 'schedule' && item.source_id != null && !isCheck) {
      await invoke('start_task_block', { sourceType: 'schedule', sourceId: String(item.source_id) });
    } else {
      await invoke('set_routine_node_status', { runId: item.runId, nodeId: item.nodeId, state: 'done' });
    }
  } else if (item.source_type === 'schedule' &&
      (item.tracking_mode === 'check' || item.marks_previous_day)) {
    await invoke('toggle_schedule_completion', {
      scheduleId: item.source_id,
      date: item.completion_date || localDate(),
    });
  } else {
    await invoke('start_task_block', { sourceType: item.source_type, sourceId: String(item.source_id) });
  }
  notifyChanged();
}

function activeHtml(block) {
  const title = block.notes || block.type_name || 'Задача';
  return `<div class="calendar-now-card__body calendar-now-card__body--active">
    <div class="calendar-now-card__copy">
      <div class="calendar-now-card__eyebrow"><span class="calendar-now-card__dot"></span>Сейчас · в работе</div>
      <div class="calendar-now-card__title">${escapeHtml(title)}</div>
      <div class="calendar-now-card__meta">Начато в ${escapeHtml(block.start_time || '—')}</div>
    </div>
    <div class="calendar-now-card__actions">
      <button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="pause">Пауза</button>
      <button class="calendar-now-card__button calendar-now-card__button--primary" data-now-action="finish">Завершить</button>
      <button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="clarify">Уточнить</button>
    </div>
  </div>`;
}

function recommendationHtml(item) {
  const duration = formatMinutes(item.durationMinutes);
  return `<div class="calendar-now-card__body">
    <div class="calendar-now-card__copy">
      <div class="calendar-now-card__eyebrow"><span class="calendar-now-card__dot"></span>Сейчас</div>
      <div class="calendar-now-card__title">${escapeHtml(item.title || 'Без названия')}</div>
      <div class="calendar-now-card__meta">${escapeHtml(item.reason)}${duration ? ` · ${duration}` : ''}</div>
    </div>
    <div class="calendar-now-card__actions">
      <button class="calendar-now-card__button calendar-now-card__button--primary" data-now-action="start">${actionLabel(item)}</button>
      <button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="dismiss">Не сейчас</button>
      <button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="clarify">Уточнить</button>
    </div>
  </div>`;
}

function emptyHtml(hadDismissed) {
  const title = hadDismissed ? 'Других подходящих действий сейчас нет' : 'На сейчас ничего не выбрано';
  const description = hadDismissed ? 'Можно вернуть рекомендации или открыть полный список.' : 'Добавьте задачу на сегодня или уточните план.';
  return `<div class="calendar-now-card__body calendar-now-card__body--empty">
    <div class="calendar-now-card__copy">
      <div class="calendar-now-card__eyebrow">Сейчас</div>
      <div class="calendar-now-card__title">${title}</div>
      <div class="calendar-now-card__meta">${description}</div>
    </div>
    <div class="calendar-now-card__actions">
      ${hadDismissed ? '<button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="restore">Показать снова</button>' : ''}
      <button class="calendar-now-card__button calendar-now-card__button--quiet" data-now-action="clarify">Уточнить</button>
    </div>
  </div>`;
}

function showError(mount, error) {
  console.error('[calendar-now]', error);
  const meta = mount.querySelector('.calendar-now-card__meta');
  if (meta) meta.textContent = 'Не удалось выполнить действие. Откройте список и попробуйте ещё раз.';
}

function wireActions(mount, activeBlock, recommendation) {
  mount.querySelectorAll('[data-now-action]').forEach(button => {
    button.addEventListener('click', async () => {
      const action = button.dataset.nowAction;
      if (action === 'clarify') { openTaskPicker(); return; }
      if (action === 'dismiss' && recommendation) {
        dismissed.add(recommendation.key);
        await renderCard();
        return;
      }
      if (action === 'restore') {
        dismissed.clear();
        await renderCard();
        return;
      }
      button.disabled = true;
      try {
        if (action === 'start' && recommendation) await runRecommendation(recommendation);
        if (action === 'pause' && activeBlock) {
          await invoke('pause_task_block', { blockId: activeBlock.id });
          notifyChanged();
        }
        if (action === 'finish' && activeBlock) {
          await invoke('complete_task_block', { blockId: activeBlock.id });
          notifyChanged();
        }
      } catch (error) {
        showError(mount, error);
        button.disabled = false;
      }
    });
  });
}

async function renderCard() {
  const mount = calendarRoot?.querySelector('#calendar-now-card');
  if (!mount) return;
  const version = ++renderVersion;
  mount.innerHTML = '<div class="calendar-now-card__loading">Выбираю следующее действие…</div>';
  const [activeBlock, data] = await Promise.all([loadActiveTaskBlock(), loadTaskRecommendationData()]);
  if (version !== renderVersion || !mount.isConnected) return;
  const recommendation = data.recommendations.find(item => !dismissed.has(item.key)) || null;
  mount.innerHTML = activeBlock ? activeHtml(activeBlock) :
    (recommendation ? recommendationHtml(recommendation) : emptyHtml(dismissed.size > 0));
  wireActions(mount, activeBlock, recommendation);
}

export function initCalendarNowCard(root) {
  calendarRoot = root;
  if (!observer) {
    observer = new MutationObserver(() => { renderCard().catch(error => console.error('[calendar-now]', error)); });
    observer.observe(root, { childList: true });
    window.addEventListener('task-state-changed', () => {
      dismissed.clear();
      renderCard().catch(error => console.error('[calendar-now]', error));
    });
    refreshTimer = setInterval(() => renderCard().catch(() => {}), 60000);
  }
  renderCard().catch(error => console.error('[calendar-now]', error));
}
