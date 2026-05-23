// ── food-cooking-log.js — Log a cooking of a recipe (date + rating + note) ──
// Opened from the calendar "+" menu. Creates a "Готовка" calendar event and an
// immutable cooking_log entry with a per-day taste rating + note.
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

export function showCookingLogModal(date, onSaved) {
  const today = date || new Date().toISOString().slice(0, 10);
  let selectedId = null, selectedName = '', rating = 0, allRecipes = [];

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal">
    <div class="modal-title">🍳 Отметить готовку</div>
    <div class="form-group"><label class="form-label">Дата</label>
      <input class="form-input" id="cl-date" type="date" value="${today}"></div>
    <div class="form-group"><label class="form-label">Рецепт</label>
      <input class="form-input" id="cl-search" placeholder="Поиск рецепта...">
      <div id="cl-list" class="mp-recipe-list" style="max-height:180px;overflow-y:auto;margin-top:6px;"></div></div>
    <div class="form-group"><label class="form-label">Оценка вкуса</label>
      <div class="rd-stars" id="cl-stars">${[1, 2, 3, 4, 5].map(n => `<span class="rd-star" data-n="${n}">★</span>`).join('')}</div></div>
    <div class="form-group"><label class="form-label">Заметка</label>
      <textarea class="form-textarea" id="cl-note" rows="2" placeholder="Как вышло, что поменять в следующий раз…"></textarea></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="cl-cancel">Отмена</button>
      <button class="btn-primary" id="cl-save" disabled>Сохранить</button>
    </div></div>`;
  document.body.appendChild(overlay);
  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('#cl-cancel').onclick = close;

  const stars = overlay.querySelectorAll('.rd-star');
  stars.forEach(s => s.onclick = () => {
    rating = parseInt(s.dataset.n);
    stars.forEach(x => x.classList.toggle('on', parseInt(x.dataset.n) <= rating));
  });

  const search = overlay.querySelector('#cl-search');
  search.oninput = () => renderList(search.value.trim().toLowerCase());
  (async () => { allRecipes = await invoke('get_recipes', { search: null }).catch(() => []); renderList(''); })();

  function renderList(q) {
    const list = overlay.querySelector('#cl-list');
    const filtered = q ? allRecipes.filter(r => r.name.toLowerCase().includes(q)) : allRecipes;
    list.innerHTML = filtered.map(r =>
      `<div class="mp-recipe-option${r.id === selectedId ? ' selected' : ''}" data-rid="${r.id}"><span>${escapeHtml(r.name)}</span></div>`
    ).join('') || '<div style="padding:8px;color:var(--text-muted);font-size:13px;">Нет рецептов</div>';
    list.querySelectorAll('.mp-recipe-option').forEach(opt => opt.onclick = () => {
      selectedId = parseInt(opt.dataset.rid);
      selectedName = allRecipes.find(r => r.id === selectedId)?.name || '';
      list.querySelectorAll('.mp-recipe-option').forEach(o => o.classList.toggle('selected', o === opt));
      overlay.querySelector('#cl-save').disabled = false;
    });
  }

  overlay.querySelector('#cl-save').onclick = async () => {
    if (!selectedId) return;
    const d = overlay.querySelector('#cl-date').value || today;
    const note = overlay.querySelector('#cl-note').value.trim();
    try {
      let color = '#cb8a05';
      const cats = await invoke('list_event_categories').catch(() => []);
      const cat = cats.find(c => c.name === 'Готовка');
      if (cat) color = cat.color;
      const eventId = await invoke('create_event', {
        title: selectedName, description: '', date: d, time: '',
        durationMinutes: 30, category: 'Готовка', color, priority: null,
      }).catch(() => null);
      await invoke('log_cooking', { recipeId: selectedId, date: d, tasteRating: rating, cookNote: note, eventId });
      close();
      if (onSaved) await onSaved();
    } catch (e) { alert('Ошибка: ' + (e.message || e)); }
  };
}
