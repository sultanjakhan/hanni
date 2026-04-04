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
    overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.5);z-index:9999;display:flex;align-items:center;justify-content:center;';

    const endTime = startTime ? nextSlot(startTime) : '23:30';
    const typeOptions = types.map(t =>
      `<option value="${t.id}" ${block && block.type_id === t.id ? 'selected' : ''}>${t.icon} ${t.name}</option>`
    ).join('');

    overlay.innerHTML = `
      <div class="modal-content" style="background:var(--bg-primary);border-radius:var(--radius-lg);padding:20px;min-width:320px;max-width:400px;border:1px solid var(--border-default);">
        <h3 style="margin:0 0 16px;font-size:15px;">${block ? 'Редактировать блок' : 'Новый блок'}</h3>
        <div style="display:flex;flex-direction:column;gap:12px;">
          <label style="font-size:13px;color:var(--text-muted);">Тип активности
            <select id="tl-modal-type" style="width:100%;padding:6px 8px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);margin-top:4px;">
              ${typeOptions}
            </select>
          </label>
          <div style="display:flex;gap:8px;">
            <label style="flex:1;font-size:13px;color:var(--text-muted);">Начало
              <input type="time" id="tl-modal-start" value="${block?.start_time || startTime || '08:00'}" step="1800" style="width:100%;padding:6px 8px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);margin-top:4px;">
            </label>
            <label style="flex:1;font-size:13px;color:var(--text-muted);">Конец
              <input type="time" id="tl-modal-end" value="${block?.end_time || endTime}" step="1800" style="width:100%;padding:6px 8px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);margin-top:4px;">
            </label>
          </div>
          <label style="font-size:13px;color:var(--text-muted);">Заметки
            <input type="text" id="tl-modal-notes" value="${block?.notes || ''}" placeholder="Необязательно" style="width:100%;padding:6px 8px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);margin-top:4px;">
          </label>
          <div style="display:flex;gap:8px;justify-content:flex-end;margin-top:8px;">
            ${block ? '<button id="tl-modal-del" style="margin-right:auto;padding:6px 12px;border-radius:var(--radius-sm);border:1px solid var(--color-red);background:none;color:var(--color-red);cursor:pointer;">Удалить</button>' : ''}
            <button id="tl-modal-cancel" style="padding:6px 12px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);cursor:pointer;">Отмена</button>
            <button id="tl-modal-save" style="padding:6px 12px;border-radius:var(--radius-sm);border:none;background:var(--accent-blue);color:#fff;cursor:pointer;">Сохранить</button>
          </div>
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
