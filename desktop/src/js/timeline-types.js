// timeline-types.js — Activity type management modal
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export async function showTypesModal() {
  const types = await invoke('get_activity_types').catch(() => []);

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.5);z-index:9999;display:flex;align-items:center;justify-content:center;';

    const renderList = (list) => list.map(t => `
      <div class="tl-type-row" data-id="${t.id}">
        <div class="tl-type-dot" style="background:${t.color}"></div>
        <span style="font-size:16px;">${t.icon}</span>
        <span class="tl-type-name">${escapeHtml(t.name)}</span>
        ${t.is_system ? '<span class="tl-type-system">системный</span>' : `<button class="tl-type-del" data-id="${t.id}" style="background:none;border:none;color:var(--color-red);cursor:pointer;font-size:14px;">✕</button>`}
      </div>`).join('');

    overlay.innerHTML = `
      <div class="modal-content" style="background:var(--bg-primary);border-radius:var(--radius-lg);padding:20px;min-width:360px;max-width:440px;border:1px solid var(--border-default);max-height:80vh;overflow:auto;">
        <h3 style="margin:0 0 12px;font-size:15px;">Типы активности</h3>
        <div id="tl-types-list">${renderList(types)}</div>
        <div style="margin-top:12px;padding-top:12px;border-top:1px solid var(--border-default);">
          <div style="font-size:13px;color:var(--text-muted);margin-bottom:8px;">Добавить тип</div>
          <div style="display:flex;gap:6px;">
            <input id="tl-new-icon" type="text" placeholder="🎯" maxlength="4" style="width:40px;padding:6px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);text-align:center;">
            <input id="tl-new-name" type="text" placeholder="Название" style="flex:1;padding:6px 8px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);">
            <input id="tl-new-color" type="color" value="#2383e2" style="width:36px;height:32px;padding:0;border:1px solid var(--border-default);border-radius:var(--radius-sm);cursor:pointer;">
            <button id="tl-add-type" style="padding:6px 12px;border-radius:var(--radius-sm);border:none;background:var(--accent-blue);color:#fff;cursor:pointer;white-space:nowrap;">+</button>
          </div>
        </div>
        <div style="text-align:right;margin-top:12px;">
          <button id="tl-types-close" style="padding:6px 16px;border-radius:var(--radius-sm);border:1px solid var(--border-default);background:var(--bg-secondary);color:var(--text-primary);cursor:pointer;">Закрыть</button>
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
      bindDeletes(overlay, close);
    });

    bindDeletes(overlay, close);

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
