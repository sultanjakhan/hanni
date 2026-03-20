// ── tab-data-people.js — People tab (contacts, blocking) ──

import { S, invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── People Tab ──
export async function loadPeople(subTab) {
  const el = document.getElementById('people-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'people', {
    title: 'People',
    subtitle: 'Контакты и связи',
    icon: '👥',
    renderDash: async (paneEl) => {
      const items = await invoke('get_contacts', {}).catch(() => []);
      const contacts = Array.isArray(items) ? items : [];
      const favs = contacts.filter(c => c.favorite).length;
      const blocked = contacts.filter(c => c.blocked).length;
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${contacts.length}</div><div class="uni-dash-label">Контактов</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${favs}</div><div class="uni-dash-label">Избранных</div></div>
          <div class="uni-dash-card purple"><div class="uni-dash-value">${blocked}</div><div class="uni-dash-label">Заблокировано</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._peopleInner || 'all';
      const filter = activeInner === 'blocked' ? { blocked: true } : {};
      try {
        const items = await invoke('get_contacts', filter);
        let contacts = Array.isArray(items) ? items : [];
        if (activeInner === 'favorites') contacts = contacts.filter(c => c.favorite);

        paneEl.innerHTML = `
          <div class="dev-filters" style="margin-bottom:var(--space-3);">
            <button class="pill${activeInner === 'all' ? ' active' : ''}" data-inner="all">Все</button>
            <button class="pill${activeInner === 'favorites' ? ' active' : ''}" data-inner="favorites">Избранные</button>
            <button class="pill${activeInner === 'blocked' ? ' active' : ''}" data-inner="blocked">Заблокированные</button>
          </div>
          <div id="people-dbv"></div>`;

        const dbvEl = paneEl.querySelector('#people-dbv');
        const dbv = new DatabaseView(dbvEl, {
          tabId: 'people',
          recordTable: 'contacts',
          records: contacts,
          fixedColumns: [
            { key: 'name', label: 'Имя', render: r => `<span class="data-table-title">${escapeHtml(r.name)}${r.favorite ? ' ★' : ''}</span>` },
            { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${r.category || r.relationship || '—'}</span>` },
            { key: 'phone', label: 'Телефон', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.phone || '—'}</span>` },
            { key: 'email', label: 'Email', render: r => `<span style="font-size:12px;color:var(--text-secondary);">${r.email || '—'}</span>` },
            { key: 'status', label: 'Статус', render: r => r.blocked ? '<span class="badge badge-red">Blocked</span>' : '<span class="badge badge-green">OK</span>' },
            { key: 'actions', label: '', render: r => `
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactFav(${r.id})">${r.favorite ? '★' : '☆'}</button>
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleContactBlock(${r.id})">${r.blocked ? '🔓' : '🚫'}</button>
              <button class="btn-secondary" style="padding:4px 8px;font-size:11px;color:var(--color-red);" onclick="deleteContact(${r.id})">✕</button>
            ` },
          ],
          idField: 'id',
          availableViews: ['table', 'list'],
          defaultView: 'table',
          addButton: '+ Добавить',
          onQuickAdd: async (name) => {
            await invoke('add_contact', { name, phone: null, email: null, category: null, relationship: null, notes: null });
            loadPeople();
          },
          reloadFn: () => loadPeople(),
        });
        await dbv.render();

        paneEl.querySelectorAll('[data-inner]').forEach(btn => {
          btn.addEventListener('click', () => { S._peopleInner = btn.dataset.inner; loadPeople(); });
        });
      } catch (e) {
        paneEl.innerHTML = `<div class="uni-empty">Ошибка: ${e}</div>`;
      }
    },
  });
}

// Window handlers for People (called from inline onclick)
window.toggleContactFav = async (id) => {
  await invoke('toggle_contact_favorite', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.toggleContactBlock = async (id) => {
  await invoke('toggle_contact_blocked', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.deleteContact = async (id) => {
  if (await confirmModal('Удалить контакт?')) {
    await invoke('delete_contact', { id });
    loadPeople(S.activeSubTab.people || 'All');
  }
};
window.deleteContactBlock = async (id) => {
  await invoke('delete_contact_block', { id });
  loadPeople(S.activeSubTab.people || 'All');
};
window.showContactBlockModal = (contactId, contactName) => {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Block site/app for ${contactName}</div>
      <div class="form-group"><label class="form-label">Type</label>
        <select class="form-select" id="cb-type" style="width:100%">
          <option value="site">Site</option><option value="app">App</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Value *</label><input class="form-input" id="cb-value" placeholder="e.g. instagram.com or Instagram"></div>
      <div class="form-group"><label class="form-label">Reason</label><input class="form-input" id="cb-reason" placeholder="Why block?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="cb-cancel">Cancel</button>
        <button class="btn-primary" id="cb-save">Add</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#cb-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#cb-save').onclick = async () => {
    const value = document.getElementById('cb-value').value.trim();
    if (!value) return;
    await invoke('add_contact_block', {
      contactId,
      blockType: document.getElementById('cb-type').value,
      value,
      reason: document.getElementById('cb-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(S.activeSubTab.people || 'All');
  };
};

function showAddContactModal() {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `
    <div class="modal">
      <div class="modal-title">Add Contact</div>
      <div class="form-group"><label class="form-label">Name *</label><input class="form-input" id="mc-name" placeholder="Name"></div>
      <div class="form-group"><label class="form-label">Phone</label><input class="form-input" id="mc-phone" placeholder="Phone"></div>
      <div class="form-group"><label class="form-label">Email</label><input class="form-input" id="mc-email" placeholder="Email"></div>
      <div class="form-group"><label class="form-label">Category</label>
        <select class="form-select" id="mc-category" style="width:100%">
          <option value="friend">Friend</option><option value="family">Family</option><option value="work">Work</option>
          <option value="spammer">Spammer</option><option value="other">Other</option>
        </select>
      </div>
      <div class="form-group"><label class="form-label">Relationship</label><input class="form-input" id="mc-rel" placeholder="e.g. College friend"></div>
      <div class="form-group"><label class="form-label">Notes</label><textarea class="form-textarea" id="mc-notes" placeholder="Notes"></textarea></div>
      <div class="form-group" style="display:flex;align-items:center;gap:8px">
        <input type="checkbox" id="mc-blocked"><label for="mc-blocked" style="color:var(--text-secondary);font-size:14px">Block this contact</label>
      </div>
      <div class="form-group"><label class="form-label">Block reason</label><input class="form-input" id="mc-reason" placeholder="Why blocked?"></div>
      <div class="modal-actions">
        <button class="btn-secondary" id="mc-cancel">Cancel</button>
        <button class="btn-primary" id="mc-save">Save</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.querySelector('#mc-cancel').onclick = () => overlay.remove();
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#mc-save').onclick = async () => {
    const name = document.getElementById('mc-name').value.trim();
    if (!name) return;
    await invoke('add_contact', {
      name,
      phone: document.getElementById('mc-phone').value.trim() || null,
      email: document.getElementById('mc-email').value.trim() || null,
      category: document.getElementById('mc-category').value,
      relationship: document.getElementById('mc-rel').value.trim() || null,
      notes: document.getElementById('mc-notes').value.trim() || null,
      blocked: document.getElementById('mc-blocked').checked,
      blockReason: document.getElementById('mc-reason').value.trim() || null,
    });
    overlay.remove();
    loadPeople(S.activeSubTab.people || 'All');
  };
}
