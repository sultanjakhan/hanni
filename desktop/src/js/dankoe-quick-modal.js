// dankoe-quick-modal.js — Quick journaling modal for Dan Koe practices opened
// from the task picker. Loads today's entry + recent history, saves text and
// marks the schedule done for the day.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const PRACTICES = {
  contemplation: {
    label: 'Contemplation', icon: '🧘', field: 'contemplation_text',
    placeholder: 'Что заметил во время созерцания? Какие мысли возвращались? Что ощущал?',
  },
  vision: {
    label: 'Vision', icon: '🔭', field: 'vision_text',
    placeholder: 'Возьми один вопрос (3 года, отказ, идеальный день…) и пиши свободно. Не редактируй — фиксируй.',
  },
  integration: {
    label: 'Integration', icon: '🔗', field: 'integration_text',
    placeholder: 'Какое одно конкретное действие сделаю сегодня?',
  },
};

const TITLE_TO_KEY = { Contemplation: 'contemplation', Vision: 'vision', Integration: 'integration' };

export function isDanKoePractice(title) {
  return Object.prototype.hasOwnProperty.call(TITLE_TO_KEY, title);
}

function localDate() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/// Open the practice modal. `scheduleId` is used to mark schedule done on save.
/// `onClose` runs after the overlay is removed (used to refresh the picker).
export async function openDanKoeModal(title, scheduleId, onClose) {
  const key = TITLE_TO_KEY[title];
  const p = PRACTICES[key];
  const today = await invoke('get_dan_koe_entry', { date: null }).catch(() => null);
  const todayText = today?.[p.field] || '';
  const history = await invoke('get_dan_koe_history', { days: 30 }).catch(() => []);
  const recent = history
    .filter(h => h.date !== localDate() && (h[p.field] || '').trim())
    .slice(0, 5);

  const overlay = document.createElement('div');
  overlay.className = 'dk-quick-overlay';
  overlay.innerHTML = `
    <div class="dk-quick-modal">
      <div class="dk-quick-header">
        <span class="dk-quick-title">${p.icon} ${p.label}</span>
        <button class="dk-quick-close" title="Закрыть">✕</button>
      </div>
      <div class="dk-quick-body">
        <textarea class="dk-quick-text" placeholder="${escapeHtml(p.placeholder)}">${escapeHtml(todayText)}</textarea>
        ${recent.length ? `
          <div class="dk-quick-history">
            <div class="dk-quick-history-title">Последние записи (${recent.length})</div>
            ${recent.map(h => `
              <div class="dk-quick-history-item">
                <div class="dk-quick-history-date">${escapeHtml(h.date)}</div>
                <div class="dk-quick-history-text">${escapeHtml((h[p.field] || '').slice(0, 400))}${(h[p.field] || '').length > 400 ? '…' : ''}</div>
              </div>`).join('')}
          </div>` : '<div class="dk-quick-empty">Истории пока нет — это первая запись.</div>'}
      </div>
      <div class="dk-quick-footer">
        <button class="dk-quick-cancel">Отмена</button>
        <button class="dk-quick-save">Сохранить</button>
      </div>
    </div>`;
  document.body.appendChild(overlay);

  const close = () => { overlay.remove(); onClose?.(); };
  overlay.querySelector('.dk-quick-close').addEventListener('click', close);
  overlay.querySelector('.dk-quick-cancel').addEventListener('click', close);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });

  overlay.querySelector('.dk-quick-save').addEventListener('click', async () => {
    const text = overlay.querySelector('.dk-quick-text').value;
    // save_dan_koe_entry overwrites all fields — keep the others as-is.
    await invoke('save_dan_koe_entry', {
      date: null,
      patternInterrupt: !!today?.pattern_interrupt,
      contemplationText: key === 'contemplation' ? text : (today?.contemplation_text || ''),
      visionText:        key === 'vision'        ? text : (today?.vision_text        || ''),
      integrationText:   key === 'integration'   ? text : (today?.integration_text   || ''),
    }).catch(() => {});
    // Mark the schedule done for today if user actually wrote something. Empty
    // save = "cancel without commitment", schedule stays open.
    if (text.trim() && scheduleId != null) {
      await invoke('toggle_schedule_completion', {
        scheduleId, date: localDate(),
      }).catch(() => {});
    }
    close();
  });

  setTimeout(() => overlay.querySelector('.dk-quick-text')?.focus(), 50);
}
