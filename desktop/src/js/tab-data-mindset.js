// ── tab-data-mindset.js — Mindset tab (journal, mood, principles) ──

import { S, invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';
import { DatabaseView } from './db-view/db-view.js';

// ── Mindset ──
export async function loadMindset(subTab) {
  const el = document.getElementById('mindset-content');
  if (!el) return;

  const { renderUnifiedLayout } = await import('./db-view/unified-layout.js');
  await renderUnifiedLayout(el, 'mindset', {
    title: 'Mindset',
    subtitle: 'Дневник, настроение, принципы',
    icon: '🧠',
    renderDash: async (paneEl) => {
      const today = await invoke('get_journal_entry', { date: null }).catch(() => null);
      const history = await invoke('get_mood_history', { days: 7 }).catch(() => []);
      const avgMood = history.length > 0 ? (history.reduce((s, m) => s + (m.mood || 3), 0) / history.length).toFixed(1) : '—';
      paneEl.innerHTML = `
        <div class="uni-dash-grid">
          <div class="uni-dash-card blue"><div class="uni-dash-value">${today ? today.mood + '/5' : '—'}</div><div class="uni-dash-label">Настроение сегодня</div></div>
          <div class="uni-dash-card green"><div class="uni-dash-value">${avgMood}</div><div class="uni-dash-label">Ср. за неделю</div></div>
          <div class="uni-dash-card yellow"><div class="uni-dash-value">${today?.energy || '—'}</div><div class="uni-dash-label">Энергия</div></div>
        </div>`;
    },
    renderTable: async (paneEl) => {
      const activeInner = S._mindsetInner || 'journal';
      paneEl.innerHTML = `
        <div class="dev-filters" style="margin-bottom:var(--space-3);">
          <button class="pill${activeInner === 'journal' ? ' active' : ''}" data-inner="journal">Дневник</button>
          <button class="pill${activeInner === 'mood' ? ' active' : ''}" data-inner="mood">Настроение</button>
          <button class="pill${activeInner === 'principles' ? ' active' : ''}" data-inner="principles">Принципы</button>
        </div>
        <div id="mindset-inner-content"></div>`;
      const innerEl = paneEl.querySelector('#mindset-inner-content');
      if (activeInner === 'mood') await loadMoodLog(innerEl);
      else if (activeInner === 'principles') await loadPrinciples(innerEl);
      else await loadJournal(innerEl);
      paneEl.querySelectorAll('[data-inner]').forEach(btn => {
        btn.addEventListener('click', () => { S._mindsetInner = btn.dataset.inner; loadMindset(); });
      });
    },
  });
}

async function loadJournal(el) {
  try {
    const today = await invoke('get_journal_entry', { date: null }).catch(() => null);
    const entries = await invoke('get_journal_entries', { days: 7 }).catch(() => []);
    const mood = today?.mood || 3, energy = today?.energy || 3, stress = today?.stress || 3;
    el.innerHTML = `
      <div class="uni-section-header">Journal</div>
      <div class="uni-form-section">
        <div class="uni-form-title">Today</div>
        <div class="settings-row"><span class="settings-label">Mood (1-5)</span><input class="form-input" id="j-mood" type="number" min="1" max="5" value="${mood}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Energy (1-5)</span><input class="form-input" id="j-energy" type="number" min="1" max="5" value="${energy}" style="width:60px;"></div>
        <div class="settings-row"><span class="settings-label">Stress (1-5)</span><input class="form-input" id="j-stress" type="number" min="1" max="5" value="${stress}" style="width:60px;"></div>
        <div class="form-group"><label class="form-label">Gratitude</label><textarea class="form-textarea" id="j-gratitude" rows="2">${escapeHtml(today?.gratitude||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Wins</label><textarea class="form-textarea" id="j-wins" rows="2">${escapeHtml(today?.wins||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Struggles</label><textarea class="form-textarea" id="j-struggles" rows="2">${escapeHtml(today?.struggles||'')}</textarea></div>
        <div class="form-group"><label class="form-label">Reflection</label><textarea class="form-textarea" id="j-reflection" rows="3">${escapeHtml(today?.reflection||'')}</textarea></div>
        <button class="btn-primary" id="j-save" style="margin-top:8px;">Save</button>
      </div>
      ${entries.length > 0 ? `<div class="uni-form-title" style="margin-top:16px;">Recent Entries</div>
        ${entries.map(e => `<div class="focus-log-item">
          <span class="focus-log-time">${e.date}</span>
          <span class="focus-log-title">Mood:${e.mood} Energy:${e.energy} Stress:${e.stress}</span>
        </div>`).join('')}` : ''}`;
    document.getElementById('j-save')?.addEventListener('click', async () => {
      try {
        await invoke('save_journal_entry', {
          mood: parseInt(document.getElementById('j-mood')?.value)||3,
          energy: parseInt(document.getElementById('j-energy')?.value)||3,
          stress: parseInt(document.getElementById('j-stress')?.value)||3,
          gratitude: document.getElementById('j-gratitude')?.value||null,
          reflection: document.getElementById('j-reflection')?.value||null,
          wins: document.getElementById('j-wins')?.value||null,
          struggles: document.getElementById('j-struggles')?.value||null,
        });
        loadJournal(el);
      } catch (err) { alert('Error: ' + err); }
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadMoodLog(el) {
  try {
    const history = await invoke('get_mood_history', { days: 14 }).catch(() => []);
    const moods = ['😤','😕','😐','🙂','😊'];
    el.innerHTML = `
      <div class="uni-section-header">Mood Log</div>
      <div style="display:flex;gap:12px;justify-content:center;margin:20px 0;">
        ${moods.map((m,i) => `<button class="mood-btn" data-mood="${i+1}" style="font-size:32px;background:none;border:none;cursor:pointer;opacity:0.5;transition:opacity 0.1s;" title="Mood ${i+1}">${m}</button>`).join('')}
      </div>
      <input class="form-input" id="mood-note" placeholder="Note (optional)..." style="max-width:400px;margin:0 auto 16px;display:block;">
      <div class="uni-form-title" style="margin-top:var(--space-3);">Recent</div>
      <div id="mood-history">
        ${history.map(m => `<div class="focus-log-item">
          <span class="focus-log-time">${m.date} ${m.time||''}</span>
          <span style="font-size:18px;">${moods[(m.mood||3)-1]}</span>
          <span class="focus-log-title">${escapeHtml(m.note||'')}</span>
        </div>`).join('')}
      </div>`;
    el.querySelectorAll('.mood-btn').forEach(btn => {
      btn.addEventListener('mouseenter', () => btn.style.opacity = '1');
      btn.addEventListener('mouseleave', () => btn.style.opacity = '0.5');
      btn.addEventListener('click', async () => {
        try {
          await invoke('log_mood', { mood: parseInt(btn.dataset.mood), note: document.getElementById('mood-note')?.value||null, trigger: null });
          loadMoodLog(el);
        } catch (err) { alert('Error: ' + err); }
      });
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}

async function loadPrinciples(el) {
  try {
    const principles = await invoke('get_principles').catch(() => []);
    const dbv = new DatabaseView(el, {
      tabId: 'mindset',
      recordTable: 'principles',
      records: principles,
      availableViews: ['table', 'list'],
      fixedColumns: [
        { key: 'active', label: '', render: r => `<div class="habit-check${r.active ? ' checked' : ''}" style="cursor:pointer;" data-pid="${r.id}">${r.active ? '&#10003;' : ''}</div>` },
        { key: 'title', label: 'Принцип', render: r => `<span class="data-table-title">${escapeHtml(r.title)}</span>` },
        { key: 'category', label: 'Категория', render: r => `<span class="badge badge-gray">${r.category || '—'}</span>` },
        { key: 'actions', label: '', render: r => `<button class="btn-secondary" style="padding:4px 8px;font-size:11px;color:var(--color-red);" data-pdel="${r.id}">✕</button>` },
      ],
      idField: 'id',
      addButton: '+ Принцип',
      onQuickAdd: async (title) => {
        await invoke('create_principle', { title, description: '', category: 'discipline' });
        loadPrinciples(el);
      },
      reloadFn: () => loadPrinciples(el),
      onDelete: async (id) => { await invoke('delete_principle', { id }); },
    });
    await dbv.render();

    // Delegate delete clicks
    el.addEventListener('click', async (e) => {
      const del = e.target.closest('[data-pdel]');
      if (del) {
        if (await confirmModal('Удалить?')) { await invoke('delete_principle', { id: parseInt(del.dataset.pdel) }).catch(()=>{}); loadPrinciples(el); }
      }
    });
  } catch (e) { el.innerHTML = `<div style="color:var(--text-muted);font-size:14px;">Error: ${e}</div>`; }
}
