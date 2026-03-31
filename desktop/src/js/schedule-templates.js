// Schedule Templates — quick-add modal for common schedule entries
const { invoke } = window.__TAURI__.core;

const TEMPLATES = [
  { cat: 'practice', label: 'DanKoe', items: [
    { title: 'Contemplation', freq: 'daily' },
    { title: 'Pattern Interrupt', freq: 'daily' },
    { title: 'Vision', freq: 'daily' },
    { title: 'Integration', freq: 'daily' },
  ]},
  { cat: 'challenge', label: 'Челлендж', items: [
    { title: 'Без мастурбации', freq: 'daily' },
    { title: 'Без порно', freq: 'daily' },
    { title: 'Без соцсетей', freq: 'daily' },
    { title: 'Без YouTube Shorts/TikTok', freq: 'daily' },
    { title: 'Без шоколада', freq: 'daily' },
    { title: 'Без газировки', freq: 'daily' },
    { title: 'Без выпечки', freq: 'daily' },
    { title: 'Без фастфуда', freq: 'daily' },
    { title: 'Без чипсов/снеков', freq: 'daily' },
    { title: 'Без мороженого', freq: 'daily' },
    { title: 'Без конфет', freq: 'daily' },
    { title: 'Без печенья', freq: 'daily' },
    { title: 'Без сахара в чай/кофе', freq: 'daily' },
    { title: 'Без энергетиков', freq: 'daily' },
    { title: 'Телефон < 1ч в день', freq: 'daily' },
    { title: 'Без телефона перед сном', freq: 'daily' },
  ]},
  { cat: 'hygiene', label: 'Гигиена', items: [
    { title: 'Зубы утром', freq: 'daily' },
    { title: 'Зубы вечером', freq: 'daily' },
    { title: 'Контрастный душ', freq: 'daily' },
    { title: 'Душ вечером', freq: 'daily' },
  ]},
  { cat: 'health', label: 'Здоровье', items: [
    { title: 'Витамины', freq: 'daily' },
    { title: 'Вода (8 стаканов)', freq: 'daily' },
    { title: 'Ел 3+ раза в день', freq: 'daily' },
    { title: 'Готовил сам', freq: 'daily' },
  ]},
  { cat: 'sport', label: 'Спорт', items: [
    { title: 'Тренировка', freq: 'daily' },
    { title: 'Растяжка', freq: 'daily' },
    { title: 'Прогулка', freq: 'daily' },
  ]},
  { cat: 'growth', label: 'Развитие', items: [
    { title: 'Изучил что-то новое', freq: 'daily' },
    { title: 'Применил новый навык', freq: 'daily' },
    { title: 'Научил/объяснил другому', freq: 'daily' },
    { title: 'Получил фидбек и осмыслил', freq: 'daily' },
  ]},
  { cat: 'home', label: 'Порядок', items: [
    { title: 'Заправил кровать', freq: 'daily' },
    { title: 'Уборка: кухня', freq: 'weekly' },
    { title: 'Уборка: ванная', freq: 'weekly' },
    { title: 'Уборка: пылесос', freq: 'weekly' },
    { title: 'Уборка: протёр пыль', freq: 'weekly' },
    { title: 'Уборка: мусор', freq: 'weekly' },
  ]},
];

const CAT_COLORS = {
  practice: 'purple', challenge: 'red', hygiene: 'pink',
  health: 'blue', sport: 'green', growth: 'yellow', home: 'orange',
};

export async function showScheduleTemplatesModal(reloadFn) {
  const existing = await invoke('get_schedules', { category: null }).catch(() => []);
  const existingTitles = new Set(existing.map(s => s.title.toLowerCase()));

  const overlay = document.createElement('div');
  overlay.className = 'modal-overlay';
  overlay.innerHTML = `<div class="modal" style="max-width:480px;max-height:80vh;overflow-y:auto;">
    <div class="modal-title">Шаблоны расписания</div>
    <div class="modal-subtitle" style="color:var(--text-muted);font-size:12px;margin-bottom:16px;">
      Выберите записи для добавления. Уже существующие отмечены серым.
    </div>
    <div id="tpl-groups"></div>
    <div class="modal-actions">
      <button class="btn-secondary" id="tpl-cancel">Отмена</button>
      <button class="btn-primary" id="tpl-save">Добавить выбранные</button>
    </div>
  </div>`;

  const groups = overlay.querySelector('#tpl-groups');
  for (const g of TEMPLATES) {
    const color = CAT_COLORS[g.cat] || 'gray';
    let html = `<div style="margin-bottom:16px;">
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px;">
        <span class="badge badge-${color}" style="font-size:12px;">${g.label}</span>
      </div>
      <div style="display:flex;flex-direction:column;gap:4px;">`;
    for (const item of g.items) {
      const exists = existingTitles.has(item.title.toLowerCase());
      html += `<label style="display:flex;align-items:center;gap:8px;font-size:13px;padding:4px 8px;border-radius:var(--radius-1);cursor:pointer;${exists ? 'opacity:0.4;' : ''}">
        <input type="checkbox" class="tpl-cb" data-cat="${g.cat}" data-title="${item.title}" data-freq="${item.freq}" ${exists ? 'disabled' : ''}>
        <span>${item.title}</span>
        ${exists ? '<span style="font-size:11px;color:var(--text-faint);">уже есть</span>' : ''}
      </label>`;
    }
    html += '</div></div>';
    groups.insertAdjacentHTML('beforeend', html);
  }

  document.body.appendChild(overlay);
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('#tpl-cancel').addEventListener('click', () => overlay.remove());

  overlay.querySelector('#tpl-save').addEventListener('click', async () => {
    const checked = overlay.querySelectorAll('.tpl-cb:checked:not(:disabled)');
    if (checked.length === 0) { overlay.remove(); return; }
    for (const cb of checked) {
      await invoke('create_schedule', {
        title: cb.dataset.title,
        category: cb.dataset.cat,
        frequency: cb.dataset.freq,
        frequencyDays: null, timeOfDay: null, details: null,
      });
    }
    overlay.remove();
    if (reloadFn) reloadFn();
  });
}
