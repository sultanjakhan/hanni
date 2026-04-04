// timeline-blocks.js — Block create/edit modal
import { invoke } from './state.js';

export async function showBlockModal(date, startTime, blockId) {
  const types = await invoke('get_activity_types').catch(() => []);
  let block = null;
  if (blockId) {
    const blocks = await invoke('get_timeline_blocks', { date }).catch(() => []);
    block = blocks.find(b => b.id === blockId);
  }

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';

    const endTime = startTime ? nextSlot(startTime) : '23:30';
    const typeOptions = types.map(t =>
      `<option value="${t.id}" ${block && block.type_id === t.id ? 'selected' : ''}>${t.icon} ${t.name}</option>`
    ).join('');

    overlay.innerHTML = `
      <div class="modal modal-compact">
        <div class="modal-title">${block ? 'Редактировать блок' : 'Новый блок'}</div>
        <div class="form-group">
          <label class="form-label">Тип активности</label>
          <select id="tl-modal-type" class="form-select">${typeOptions}</select>
        </div>
        <div class="form-row">
          <div class="form-group" style="flex:1;">
            <label class="form-label">Начало</label>
            <input type="time" id="tl-modal-start" class="form-input" value="${block?.start_time || startTime || '08:00'}" step="1800">
          </div>
          <div class="form-group" style="flex:1;">
            <label class="form-label">Конец</label>
            <input type="time" id="tl-modal-end" class="form-input" value="${block?.end_time || endTime}" step="1800">
          </div>
        </div>
        <div class="form-group">
          <label class="form-label">Заметки</label>
          <input type="text" id="tl-modal-notes" class="form-input" value="${block?.notes || ''}" placeholder="Необязательно">
        </div>
        <div class="modal-actions">
          ${block ? '<button id="tl-modal-del" class="btn-danger" style="margin-right:auto;">Удалить</button>' : ''}
          <button id="tl-modal-cancel" class="btn-secondary">Отмена</button>
          <button id="tl-modal-save" class="btn-primary">Сохранить</button>
        </div>
      </div>`;

    document.body.appendChild(overlay);

    const close = () => { overlay.remove(); resolve(); };
    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
    overlay.querySelector('#tl-modal-cancel').addEventListener('click', close);

    overlay.querySelector('#tl-modal-del')?.addEventListener('click', async () => {
      await invoke('delete_timeline_block', { id: blockId });
      close();
    });

    overlay.querySelector('#tl-modal-save').addEventListener('click', async () => {
      const typeId = parseInt(overlay.querySelector('#tl-modal-type').value);
      const start = overlay.querySelector('#tl-modal-start').value;
      const end = overlay.querySelector('#tl-modal-end').value;
      const notes = overlay.querySelector('#tl-modal-notes').value;
      if (!start || !end || start >= end) { alert('Время начала должно быть раньше конца'); return; }
      if (block) {
        await invoke('update_timeline_block', { id: blockId, typeId, startTime: start, endTime: end, notes });
      } else {
        await invoke('create_timeline_block', { typeId, date, startTime: start, endTime: end, notes });
      }
      close();
    });
  });
}

function nextSlot(time) {
  const [h, m] = time.split(':').map(Number);
  const next = h * 60 + m + 30;
  return `${String(Math.floor(next / 60) % 24).padStart(2, '0')}:${String(next % 60).padStart(2, '0')}`;
}
