// task-weights-editor.js — Inline editor for per-category importance weights.
// Opened from the picker's gear. Persists JSON in app_settings.task_category_weights;
// the picker's composite score reads it. Weights are 1..5 (1 = neutral).
import { invoke } from './state.js';

const CATS = [
  ['health', '💚', 'Здоровье'], ['sport', '🔥', 'Спорт'], ['hygiene', '🫧', 'Гигиена'],
  ['home', '🏡', 'Дом'], ['practice', '🎯', 'Практика'], ['challenge', '⚡', 'Челлендж'],
  ['growth', '🌱', 'Развитие'], ['work', '⚙️', 'Работа'], ['other', '◽', 'Другое'],
];

function dotsHtml(weight) {
  return [1, 2, 3, 4, 5]
    .map(i => `<span class="rt-nm-dot${i <= weight ? ' on' : ''}" data-w="${i}"></span>`).join('');
}

/// Replace the panel content with the weights editor. `onDone` re-opens the picker.
export async function openWeightsEditor(panel, onDone) {
  let weights = {};
  try {
    const raw = await invoke('get_app_setting', { key: 'task_category_weights' });
    weights = raw ? JSON.parse(raw) : {};
  } catch { weights = {}; }

  const rows = CATS.map(([cat, icon, label]) => `
    <div class="tw-w-row" data-cat="${cat}">
      <span class="tw-item-icon">${icon}</span>
      <span class="tw-item-title">${label}</span>
      <span class="rt-nm-dots tw-w-dots">${dotsHtml(weights[cat] ?? 1)}</span>
    </div>`).join('');

  panel.innerHTML = `
    <div class="tw-panel-header tw-weights-header">
      <button class="tw-back" title="Назад">←</button>
      <span>Важность категорий</span>
    </div>
    <div class="tw-panel-body">${rows}</div>`;

  panel.querySelector('.tw-back').addEventListener('click', (e) => { e.stopPropagation(); onDone(); });

  panel.querySelectorAll('.tw-w-dots .rt-nm-dot').forEach(dot => {
    dot.addEventListener('click', async (e) => {
      e.stopPropagation();
      const row = dot.closest('.tw-w-row');
      const w = parseInt(dot.dataset.w);
      weights[row.dataset.cat] = w;
      row.querySelector('.tw-w-dots').innerHTML = dotsHtml(w);
      await invoke('set_app_setting', {
        key: 'task_category_weights', value: JSON.stringify(weights),
      }).catch(() => {});
    });
  });
}
