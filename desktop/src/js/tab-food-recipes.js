// ── tab-food-recipes.js — Recipe book pane for Food tab ──
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';

const MEAL_FILTERS = [
  { id: 'all', label: 'Все' },
  { id: 'breakfast', label: 'Завтрак' },
  { id: 'lunch', label: 'Обед' },
  { id: 'dinner', label: 'Ужин' },
];

const MEAL_COLORS = { breakfast: 'yellow', lunch: 'green', dinner: 'purple', universal: 'blue' };

async function getBlacklist() {
  try {
    const entries = await invoke('memory_list', { category: 'food', limit: 100 });
    const items = [];
    for (const e of entries) {
      const k = e.key.toLowerCase();
      if (k.includes('блэклист') || k.includes('blacklist') || k.includes('аллергия') || k.includes('allergy')) {
        items.push(...e.value.split(',').map(s => s.trim().toLowerCase()).filter(Boolean));
      }
    }
    return items;
  } catch { return []; }
}

function matchesBlacklist(recipe, blacklist) {
  if (!blacklist.length) return false;
  const text = `${recipe.name} ${recipe.ingredients || ''} ${recipe.description || ''}`.toLowerCase();
  return blacklist.some(item => text.includes(item));
}

function matchesMealFilter(recipe, filter) {
  if (filter === 'all') return true;
  return (recipe.tags || '').split(',').map(t => t.trim()).includes(filter);
}

function matchesSearch(recipe, query) {
  if (!query) return true;
  const text = `${recipe.name} ${recipe.ingredients || ''}`.toLowerCase();
  return text.includes(query);
}

function getIngrNames(recipe) {
  return (recipe.ingredients || '').split(',').map(s => {
    const colonIdx = s.indexOf(':');
    return (colonIdx > -1 ? s.slice(0, colonIdx) : s).trim();
  }).filter(Boolean);
}

function renderCard(r, onIngrClick) {
  const tags = (r.tags || '').split(',').map(t => t.trim()).filter(Boolean);
  const badgesHtml = tags.map(t => {
    const c = MEAL_COLORS[t] || 'gray';
    const label = { breakfast: 'Завтрак', lunch: 'Обед', dinner: 'Ужин', universal: 'Универсал' }[t] || t;
    return `<span class="badge badge-${c}">${label}</span>`;
  }).join('');
  const totalTime = (r.prep_time || 0) + (r.cook_time || 0);
  const diffLabel = r.difficulty === 'medium' ? 'Средний' : 'Лёгкий';
  const ingrNames = getIngrNames(r);
  const ingrHtml = ingrNames.slice(0, 5).map(n =>
    `<span class="ingr-tag" data-ingr="${escapeHtml(n)}">${escapeHtml(n)}</span>`
  ).join('') + (ingrNames.length > 5 ? `<span class="ingr-tag ingr-more">+${ingrNames.length - 5}</span>` : '');

  const div = document.createElement('div');
  div.className = 'recipe-card';
  div.dataset.id = r.id;
  div.innerHTML = `
    <div class="recipe-card-header">
      <span class="recipe-card-name">${escapeHtml(r.name)}</span>
      <span class="recipe-card-cal">${r.calories || '—'} kcal</span>
    </div>
    <div class="recipe-card-meta">
      ${totalTime ? `<span>⏱ ${totalTime} мин</span>` : ''}
      <span>👥 ${r.servings || 1}</span>
      <span class="recipe-diff recipe-diff-${r.difficulty || 'easy'}">${diffLabel}</span>
    </div>
    <div class="recipe-card-tags">${badgesHtml}</div>
    <div class="recipe-card-ingr">${ingrHtml}</div>`;

  div.querySelectorAll('.ingr-tag[data-ingr]').forEach(tag => {
    tag.addEventListener('click', (e) => {
      e.stopPropagation();
      onIngrClick(tag.dataset.ingr);
    });
  });
  return div;
}

export async function renderRecipesPane(el) {
  let activeFilter = 'all';
  let searchQuery = '';

  async function render() {
    const [recipes, blacklist] = await Promise.all([
      invoke('get_recipes', { search: null }).catch(() => []),
      getBlacklist(),
    ]);

    const filtered = recipes
      .filter(r => !matchesBlacklist(r, blacklist))
      .filter(r => matchesMealFilter(r, activeFilter))
      .filter(r => matchesSearch(r, searchQuery));

    el.innerHTML = `
      <div class="recipe-pane">
        <div class="recipe-filter-bar">
          <div class="recipe-filters">${MEAL_FILTERS.map(f =>
            `<button class="recipe-filter-btn${activeFilter === f.id ? ' active' : ''}" data-filter="${f.id}">${f.label}</button>`
          ).join('')}</div>
          <input class="recipe-search" type="text" placeholder="Поиск по названию или продукту..." value="${escapeHtml(searchQuery)}">
          <button class="btn-primary recipe-add-btn">+ Рецепт</button>
        </div>
        <div class="recipe-grid"></div>
      </div>`;

    const grid = el.querySelector('.recipe-grid');
    if (!filtered.length) {
      grid.innerHTML = '<div class="uni-empty">Нет рецептов</div>';
    } else {
      for (const r of filtered) {
        const card = renderCard(r, (ingr) => {
          searchQuery = ingr.toLowerCase();
          const input = el.querySelector('.recipe-search');
          if (input) input.value = searchQuery;
          render();
        });
        card.addEventListener('click', async () => {
          const { showRecipeDetail } = await import('./food-recipe-modals.js');
          showRecipeDetail(parseInt(card.dataset.id), render);
        });
        grid.appendChild(card);
      }
    }

    el.querySelectorAll('.recipe-filter-btn').forEach(btn => {
      btn.onclick = () => { activeFilter = btn.dataset.filter; render(); };
    });
    const searchInput = el.querySelector('.recipe-search');
    if (searchInput) {
      searchInput.oninput = () => { searchQuery = searchInput.value.trim().toLowerCase(); render(); };
    }
    el.querySelector('.recipe-add-btn')?.addEventListener('click', async () => {
      const { showAddRecipeModal } = await import('./food-recipe-modals.js');
      showAddRecipeModal(render);
    });
  }

  await render();
}
