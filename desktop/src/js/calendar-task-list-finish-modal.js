// "Как прошло?" — finish modal shown after ✓ Готово.
// Collects quality (0..5 stars), mood (happy/neutral/sad), free reflection text.
// On Save → invoke('finish_task_block', {blockId, quality, reflection, mood}).
// On Skip → finish without reflection (still closes the block + marks source done).

import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const MOOD_OPTS = [
  { value: 'happy',   icon: '😊', label: 'Хорошо' },
  { value: 'neutral', icon: '😐', label: 'Норм' },
  { value: 'sad',     icon: '😕', label: 'Плохо' },
];

export function showFinishModal(blockId, taskTitle, onDone) {
  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  const stars = [1,2,3,4,5].map(n =>
    `<button class="ctl-fin-star" data-star="${n}" type="button" aria-label="${n}">★</button>`
  ).join('');
  const moods = MOOD_OPTS.map(m =>
    `<button class="ctl-fin-mood" data-mood="${m.value}" type="button" title="${m.label}">${m.icon}</button>`
  ).join('');
  overlay.innerHTML = `<div class="modal modal-compact ctl-fin-modal">
    <div class="modal-title">Как прошло?</div>
    ${taskTitle ? `<div class="ctl-fin-task">${escapeHtml(taskTitle)}</div>` : ''}
    <div class="ctl-fin-section">
      <label class="ctl-fin-label">Качество</label>
      <div class="ctl-fin-stars" id="ctl-fin-stars">${stars}</div>
    </div>
    <div class="ctl-fin-section">
      <label class="ctl-fin-label">Настроение</label>
      <div class="ctl-fin-moods" id="ctl-fin-moods">${moods}</div>
    </div>
    <div class="ctl-fin-section">
      <label class="ctl-fin-label">Заметка-рефлексия</label>
      <textarea class="form-textarea" id="ctl-fin-reflection" placeholder="Как прошло, что мешало, что заметил…" rows="3"></textarea>
    </div>
    <div class="modal-actions">
      <button class="btn-secondary" id="ctl-fin-skip">Пропустить</button>
      <button class="btn-primary" id="ctl-fin-save">Сохранить</button>
    </div>
  </div>`;
  document.body.appendChild(overlay);

  let quality = 0;
  let mood = null;
  const starsEl = overlay.querySelector('#ctl-fin-stars');
  const moodsEl = overlay.querySelector('#ctl-fin-moods');
  const textEl = overlay.querySelector('#ctl-fin-reflection');

  const paintStars = () => starsEl.querySelectorAll('[data-star]').forEach(b => {
    b.classList.toggle('on', parseInt(b.dataset.star) <= quality);
  });
  starsEl.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-star]');
    if (!btn) return;
    const n = parseInt(btn.dataset.star);
    quality = (quality === n) ? 0 : n; // click same star = clear
    paintStars();
  });
  moodsEl.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-mood]');
    if (!btn) return;
    mood = (mood === btn.dataset.mood) ? null : btn.dataset.mood;
    moodsEl.querySelectorAll('[data-mood]').forEach(b => b.classList.toggle('on', b.dataset.mood === mood));
  });

  const close = () => overlay.remove();
  const submit = async (withReflection) => {
    const args = { blockId };
    if (withReflection) {
      if (quality > 0) args.quality = quality;
      if (mood) args.mood = mood;
      const t = (textEl.value || '').trim();
      if (t) args.reflection = t;
    }
    try { await invoke('finish_task_block', args); }
    catch (err) { console.error('finish_task_block:', err); }
    close();
    onDone && onDone();
  };

  overlay.addEventListener('click', (e) => { if (e.target === overlay) submit(false); });
  overlay.querySelector('#ctl-fin-skip').addEventListener('click', () => submit(false));
  overlay.querySelector('#ctl-fin-save').addEventListener('click', () => submit(true));
  overlay.addEventListener('keydown', (e) => { if (e.key === 'Escape') submit(false); });
  textEl.focus();
}
