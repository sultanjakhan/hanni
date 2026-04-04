// timeline-types.js — Activity type management modal
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export async function showTypesModal() {
  const types = await invoke('get_activity_types').catch(() => []);

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';

    const renderList = (list) => list.map(t => `
      <div class="tl-type-row" data-id="${t.id}">
        <div class="tl-type-dot" style="background:${t.color}"></div>
        <span style="font-size:16px;">${t.icon}</span>
        <span class="tl-type-name">${escapeHtml(t.name)}</span>
        ${t.is_system ? '<span class="tl-type-system">системный</span>' : `<button class="tl-type-del" data-id="${t.id}" style="background:none;border:none;color:var(--color-red);cursor:pointer;font-size:14px;">✕</button>`}
      </div>`).join('');

    overlay.innerHTML = `
      <div class="modal modal-compact">
        <div class="modal-title">Типы активности</div>
        <div id="tl-types-list">${renderList(types)}</div>
        <div class="form-group" style="margin-top:var(--space-4);padding-top:var(--space-3);border-top:1px solid var(--border-default);">
          <div class="form-label">Добавить тип</div>
          <div class="form-row">
            <input id="tl-new-icon" class="form-input" type="text" placeholder="🎯" maxlength="4" style="width:40px;flex:none;text-align:center;">
            <input id="tl-new-name" class="form-input" type="text" placeholder="Название">
            <input id="tl-new-color" type="color" value="#2383e2" style="width:36px;height:32px;padding:0;border:1px solid var(--border-default);border-radius:var(--radius-sm);cursor:pointer;">
            <button id="tl-add-type" class="btn-primary" style="padding:6px 12px;">+</button>
          </div>
        </div>
        <div class="modal-actions">
          <button id="tl-types-close" class="btn-secondary">Закрыть</button>
        </div>
      </div>`;

    document.body.appendChild(overlay);
    const close = () => { overlay.remove(); resolve(); };

    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
    overlay.querySelector('#tl-types-close').addEventListener('click', close);

    overlay.querySelector('#tl-add-type').addEventListener('click', async () => {
      const name = overlay.querySelector('#tl-new-name').value.trim();
      const icon = overlay.querySelector('#tl-new-icon').value.trim() || '📌';
      const color = overlay.querySelector('#tl-new-color').value;
      if (!name) return;
      await invoke('create_activity_type', { name, color, icon });
      const fresh = await invoke('get_activity_types').catch(() => []);
      overlay.querySelector('#tl-types-list').innerHTML = renderList(fresh);
      overlay.querySelector('#tl-new-name').value = '';
      overlay.querySelector('#tl-new-icon').value = '';
      bindDeletes(overlay);
    });

    bindDeletes(overlay);

    async function bindDeletes(ov) {
      ov.querySelectorAll('.tl-type-del').forEach(btn => {
        btn.addEventListener('click', async (e) => {
          e.stopPropagation();
          await invoke('delete_activity_type', { id: parseInt(btn.dataset.id) });
          const fresh = await invoke('get_activity_types').catch(() => []);
          ov.querySelector('#tl-types-list').innerHTML = renderList(fresh);
          bindDeletes(ov);
        });
      });
    }
  });
}
