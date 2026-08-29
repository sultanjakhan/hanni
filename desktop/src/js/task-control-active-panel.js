// Active task actions shown by the floating task-control widget.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export function createActiveTaskPanel(activeBlock, onComplete) {
  const label = activeBlock.notes || activeBlock.type_name || 'Задача';
  const panel = document.createElement('div');
  panel.className = 'tw-panel tw-panel-actions';
  panel.innerHTML = `
    <div class="tw-panel-header">Идёт: ${escapeHtml(label)} с ${activeBlock.start_time}</div>
    <div class="tw-panel-body">
      <button class="tw-action tw-action-pause" data-action="pause">
        <span class="tw-action-icon">⏸</span>
        <span class="tw-action-label">Пауза</span>
        <span class="tw-action-hint">блок закрывается, статус не меняется</span>
      </button>
      <button class="tw-action tw-action-finish" data-action="finish">
        <span class="tw-action-icon">✓</span>
        <span class="tw-action-label">Завершить</span>
        <span class="tw-action-hint">отметить как сделано</span>
      </button>
      <button class="tw-action tw-action-cancel" data-action="cancel">
        <span class="tw-action-icon">✕</span>
        <span class="tw-action-label">Отмена</span>
        <span class="tw-action-hint">удалить блок без зачёта времени</span>
      </button>
    </div>`;

  panel.querySelectorAll('[data-action]').forEach(button => {
    button.addEventListener('click', async () => {
      const action = button.dataset.action;
      try {
        if (action === 'pause') await invoke('pause_task_block', { blockId: activeBlock.id });
        if (action === 'finish') await invoke('complete_task_block', { blockId: activeBlock.id });
        if (action === 'cancel') await invoke('delete_timeline_block', { id: activeBlock.id });
      } catch (error) {
        console.error('task action:', error);
      }
      await onComplete();
    });
  });
  return panel;
}
