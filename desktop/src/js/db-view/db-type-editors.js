// ── db-view/db-type-editors.js — Custom editors for time, progress, rating ──

/** Time editor — HH:MM 24h input */
export function renderTimeEditor(cell, value, saveFn) {
  cell.style.position = 'relative';
  Array.from(cell.children).forEach(c => c.style.visibility = 'hidden');

  const wrap = document.createElement('div');
  wrap.className = 'inline-editor time-editor';

  const hh = document.createElement('input');
  hh.type = 'text'; hh.maxLength = 2; hh.placeholder = 'ЧЧ'; hh.className = 'time-part';
  const sep = document.createElement('span');
  sep.textContent = ':'; sep.className = 'time-sep';
  const mm = document.createElement('input');
  mm.type = 'text'; mm.maxLength = 2; mm.placeholder = 'ММ'; mm.className = 'time-part';

  const [h, m] = (value || '').split(':');
  hh.value = h || ''; mm.value = m || '';

  wrap.append(hh, sep, mm);
  cell.appendChild(wrap);
  hh.focus(); hh.select();

  const save = () => {
    const hours = hh.value.padStart(2, '0');
    const mins = mm.value.padStart(2, '0');
    const h = parseInt(hours), mi = parseInt(mins);
    const valid = !isNaN(h) && !isNaN(mi) && h >= 0 && h <= 23 && mi >= 0 && mi <= 59;
    wrap.remove();
    Array.from(cell.children).forEach(c => c.style.visibility = '');
    cell.style.position = '';
    saveFn(valid ? `${hours}:${mins}` : value || '');
  };

  hh.addEventListener('input', () => { if (hh.value.length === 2) mm.focus(); });
  mm.addEventListener('keydown', (e) => {
    if (e.key === 'Backspace' && mm.value === '') hh.focus();
    if (e.key === 'Enter') { e.preventDefault(); save(); }
    if (e.key === 'Escape') { wrap.remove(); Array.from(cell.children).forEach(c => c.style.visibility = ''); cell.style.position = ''; }
    e.stopPropagation();
  });
  hh.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); save(); }
    if (e.key === 'Escape') { wrap.remove(); Array.from(cell.children).forEach(c => c.style.visibility = ''); cell.style.position = ''; }
    e.stopPropagation();
  });
  mm.addEventListener('blur', (e) => { if (!wrap.contains(e.relatedTarget)) save(); });
  hh.addEventListener('blur', (e) => { if (!wrap.contains(e.relatedTarget)) save(); });
}

/** Progress editor — range slider 0-100 */
export function renderProgressEditor(cell, value, saveFn) {
  cell.style.position = 'relative';
  Array.from(cell.children).forEach(c => c.style.visibility = 'hidden');

  const wrap = document.createElement('div');
  wrap.className = 'inline-editor progress-editor';

  const slider = document.createElement('input');
  slider.type = 'range'; slider.min = '0'; slider.max = '100'; slider.value = parseInt(value) || 0;
  slider.className = 'progress-slider';

  const num = document.createElement('span');
  num.className = 'progress-num';
  num.textContent = `${slider.value}%`;

  slider.addEventListener('input', () => { num.textContent = `${slider.value}%`; });

  wrap.append(slider, num);
  cell.appendChild(wrap);
  slider.focus();

  const save = () => {
    const val = slider.value;
    wrap.remove();
    Array.from(cell.children).forEach(c => c.style.visibility = '');
    cell.style.position = '';
    saveFn(val);
  };

  slider.addEventListener('change', save);
  slider.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); save(); }
    if (e.key === 'Escape') { wrap.remove(); Array.from(cell.children).forEach(c => c.style.visibility = ''); cell.style.position = ''; }
    e.stopPropagation();
  });
  slider.addEventListener('blur', save);
}

/** Rating editor — 1-5 stars */
export function renderRatingEditor(cell, value, saveFn) {
  cell.style.position = 'relative';
  Array.from(cell.children).forEach(c => c.style.visibility = 'hidden');

  const wrap = document.createElement('div');
  wrap.className = 'inline-editor rating-editor';

  const current = parseInt(value) || 0;
  for (let i = 1; i <= 5; i++) {
    const star = document.createElement('span');
    star.className = `rating-star${i <= current ? ' filled' : ''}`;
    star.textContent = '★';
    star.dataset.val = i;
    star.addEventListener('click', (e) => {
      e.stopPropagation();
      const val = i === current ? '' : String(i);
      wrap.remove();
      Array.from(cell.children).forEach(c => c.style.visibility = '');
      cell.style.position = '';
      saveFn(val);
    });
    wrap.appendChild(star);
  }
  cell.appendChild(wrap);

  const close = (e) => {
    if (!wrap.contains(e.target) && e.target !== wrap) {
      wrap.remove();
      Array.from(cell.children).forEach(c => c.style.visibility = '');
      cell.style.position = '';
      document.removeEventListener('mousedown', close);
    }
  };
  setTimeout(() => document.addEventListener('mousedown', close), 10);
}
