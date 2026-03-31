// ── db-view/db-recurrence-editor.js — Recurrence picker popover ──

const UNITS = [
  { value: 'hour', label: 'час', labelN: 'часов' },
  { value: 'day', label: 'день', labelN: 'дней' },
  { value: 'week', label: 'неделя', labelN: 'недель' },
  { value: 'month', label: 'месяц', labelN: 'месяцев' },
];
const DAY_LABELS = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];
const TIMES = (() => {
  const t = [];
  for (let h = 0; h < 24; h++) for (const m of ['00', '30']) t.push(`${String(h).padStart(2, '0')}:${m}`);
  return t;
})();

function parseRecurrence(raw) {
  if (!raw) return { every: 1, unit: 'day', days: [], time: '09:00', until: '' };
  try {
    const o = JSON.parse(raw);
    return { every: o.every || 1, unit: o.unit || 'day', days: o.days || [], time: o.time || '', until: o.until || '' };
  } catch { return { every: 1, unit: 'day', days: [], time: '09:00', until: '' }; }
}

export function formatRecurrence(raw) {
  const r = parseRecurrence(raw);
  const timePart = r.time ? ` · ${r.time}` : '';
  let text = '';
  if (r.unit === 'hour') {
    text = r.every === 1 ? 'Каждый час' : `Каждые ${r.every}ч`;
  } else if (r.unit === 'day') {
    text = r.every === 1 ? `Ежедневно${timePart}` : `Каждые ${r.every} дн.${timePart}`;
  } else if (r.unit === 'week') {
    if (r.days?.length > 0) {
      text = r.days.sort((a, b) => a - b).map(d => DAY_LABELS[d - 1] || d).join(', ') + timePart;
    } else {
      text = r.every === 1 ? `Еженедельно${timePart}` : `Каждые ${r.every} нед.${timePart}`;
    }
  } else if (r.unit === 'month') {
    text = r.every === 1 ? `Ежемесячно${timePart}` : `Каждые ${r.every} мес.${timePart}`;
  } else {
    return raw || '—';
  }
  if (r.until) {
    const d = new Date(r.until);
    if (!isNaN(d)) text += ` → ${d.toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' })}`;
  }
  return text;
}

export function showRecurrenceEditor(cell, currentVal, save) {
  document.querySelectorAll('.recurrence-editor').forEach(d => d.remove());
  const r = parseRecurrence(currentVal);
  const rect = cell.getBoundingClientRect();

  const dd = document.createElement('div');
  dd.className = 'recurrence-editor';
  dd.style.left = rect.left + 'px';
  dd.style.top = rect.bottom + 4 + 'px';
  dd.style.minWidth = Math.max(rect.width, 300) + 'px';

  const render = () => {
    const showDays = r.unit === 'week';
    const showTime = r.unit !== 'hour';
    const unitOpts = UNITS.map(u =>
      `<option value="${u.value}"${u.value === r.unit ? ' selected' : ''}>${r.every > 1 ? u.labelN : u.label}</option>`
    ).join('');
    const timeOpts = TIMES.map(t => `<option value="${t}"${t === r.time ? ' selected' : ''}>${t}</option>`).join('');

    dd.innerHTML = `
      <div class="rec-section">
        <div class="rec-section-label">Частота</div>
        <div class="rec-freq-row">
          <span class="rec-freq-prefix">Каждые</span>
          <input type="number" class="rec-every" min="1" max="99" value="${r.every}">
          <select class="rec-unit">${unitOpts}</select>
        </div>
      </div>
      ${showDays ? `
      <div class="rec-section">
        <div class="rec-section-label">Дни недели</div>
        <div class="rec-days-row">${DAY_LABELS.map((l, i) =>
          `<button class="rec-day-btn${r.days.includes(i + 1) ? ' active' : ''}${i >= 5 ? ' weekend' : ''}" data-day="${i + 1}">${l}</button>`
        ).join('')}</div>
      </div>` : ''}
      ${showTime ? `
      <div class="rec-section">
        <div class="rec-section-label">Время</div>
        <div class="rec-time-row">
          <select class="rec-time">${timeOpts}</select>
        </div>
      </div>` : ''}
      <div class="rec-section">
        <div class="rec-section-label">До даты <span class="rec-hint">(необязательно)</span></div>
        <input type="date" class="rec-until" value="${r.until || ''}">
      </div>
      <div class="rec-footer"><button class="rec-clear">Очистить</button><button class="rec-save">Готово</button></div>`;

    dd.querySelector('.rec-every').addEventListener('input', (e) => {
      r.every = Math.max(1, parseInt(e.target.value) || 1);
      markEdited();
      dd.querySelector('.rec-unit').innerHTML = UNITS.map(u =>
        `<option value="${u.value}"${u.value === r.unit ? ' selected' : ''}>${r.every > 1 ? u.labelN : u.label}</option>`
      ).join('');
    });
    dd.querySelector('.rec-unit').addEventListener('change', (e) => {
      r.unit = e.target.value;
      if (r.unit !== 'week') r.days = [];
      markEdited();
      render();
    });
    dd.querySelectorAll('.rec-day-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const d = parseInt(btn.dataset.day);
        const idx = r.days.indexOf(d);
        if (idx >= 0) r.days.splice(idx, 1); else r.days.push(d);
        btn.classList.toggle('active');
        markEdited();
      });
    });
    dd.querySelector('.rec-time')?.addEventListener('change', (e) => { r.time = e.target.value; markEdited(); });
    dd.querySelector('.rec-until')?.addEventListener('change', (e) => { r.until = e.target.value; markEdited(); });
    dd.querySelector('.rec-clear').addEventListener('click', () => {
      save('');
      closeDd();
    });
    dd.querySelector('.rec-save').addEventListener('click', () => {
      const val = { every: r.every, unit: r.unit };
      if (r.unit === 'week' && r.days.length > 0) val.days = r.days.sort((a, b) => a - b);
      if (r.unit !== 'hour' && r.time) val.time = r.time;
      if (r.until) val.until = r.until;
      save(JSON.stringify(val));
      closeDd();
    });
  };

  const origJson = currentVal || '';
  let userEdited = false;
  const markEdited = () => { userEdited = true; };
  const closeRef = { fn: null };
  const closeDd = () => {
    dd.remove();
    if (closeRef.fn) { document.removeEventListener('mousedown', closeRef.fn); closeRef.fn = null; }
    if (closeRef.esc) { document.removeEventListener('keydown', closeRef.esc); closeRef.esc = null; }
  };
  render();
  document.body.appendChild(dd);
  const ddRect = dd.getBoundingClientRect();
  if (ddRect.bottom > window.innerHeight - 8) {
    dd.style.top = Math.max(4, rect.top - ddRect.height - 4) + 'px';
  }
  if (ddRect.right > window.innerWidth - 8) {
    dd.style.left = Math.max(4, window.innerWidth - ddRect.width - 8) + 'px';
  }
  closeRef.esc = (e) => { if (e.key === 'Escape') closeDd(); };
  document.addEventListener('keydown', closeRef.esc);

  setTimeout(() => {
    closeRef.fn = (e) => {
      if (dd.contains(e.target)) return;
      const val = { every: r.every, unit: r.unit };
      if (r.unit === 'week' && r.days.length > 0) val.days = r.days.sort((a, b) => a - b);
      if (r.unit !== 'hour' && r.time) val.time = r.time;
      if (r.until) val.until = r.until;
      if (userEdited) save(JSON.stringify(val));
      closeDd();
    };
    document.addEventListener('mousedown', closeRef.fn);
  }, 10);
}
