// ── sport-template-filters.js — Filter logic for workout templates ──

export const WORKOUT_TYPES = [
  { id: 'all', label: 'Все' }, { id: 'gym', label: 'Зал' },
  { id: 'cardio', label: 'Кардио' }, { id: 'yoga', label: 'Йога' },
  { id: 'swimming', label: 'Плавание' }, { id: 'martial_arts', label: 'Единоборства' },
  { id: 'other', label: 'Другое' },
];

export const DIFFS = [
  { id: 'all', label: 'Любая' }, { id: 'easy', label: 'Лёгкий' },
  { id: 'medium', label: 'Средний' }, { id: 'hard', label: 'Сложный' },
];

export const TYPE_COLORS = {
  gym: 'blue', cardio: 'red', yoga: 'green', swimming: 'purple',
  martial_arts: 'orange', other: 'gray',
};

export const DIFF_COLORS = { easy: 'green', medium: 'yellow', hard: 'red' };

export const matchType = (t, f) => f === 'all' || t.type === f;
export const matchDiff = (t, f) => f === 'all' || t.difficulty === f;
export const matchSearch = (t, q) => !q || `${t.name} ${t.notes || ''}`.toLowerCase().includes(q);
export const matchMuscle = (t, f) => f === 'all' || (t.target_muscle_groups || '').split(',').some(m => m.trim() === f);

export function collectMuscleGroups(templates) {
  const set = new Set();
  for (const t of templates) {
    for (const m of (t.target_muscle_groups || '').split(',')) {
      const trimmed = m.trim();
      if (trimmed) set.add(trimmed);
    }
  }
  return set;
}
