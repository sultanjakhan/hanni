// ── sport-catalog-filters.js — Filter logic for exercise catalog ──
import { invoke } from './state.js';

export const MUSCLE_GROUPS = [
  { id: 'all', label: 'Все' }, { id: 'chest', label: 'Грудь' },
  { id: 'back', label: 'Спина' }, { id: 'shoulders', label: 'Плечи' },
  { id: 'biceps', label: 'Бицепс' }, { id: 'triceps', label: 'Трицепс' },
  { id: 'legs', label: 'Ноги' }, { id: 'core', label: 'Кор' },
  { id: 'full_body', label: 'Всё тело' },
];

export const EXERCISE_TYPES = [
  { id: 'all', label: 'Все' }, { id: 'strength', label: 'Силовое' },
  { id: 'cardio', label: 'Кардио' }, { id: 'stretch', label: 'Растяжка' },
  { id: 'bodyweight', label: 'С весом тела' },
];

let _catalogCache = null;
export async function loadExerciseCatalog() {
  try { _catalogCache = await invoke('get_exercise_catalog', { search: null }); }
  catch { _catalogCache = []; }
  return _catalogCache;
}
export function getCatalogCache() { return _catalogCache || []; }
export function invalidateCatalogCache() { _catalogCache = null; }

export const matchMuscle = (ex, f) => f === 'all' || ex.muscle_group === f;
export const matchType = (ex, f) => f === 'all' || ex.type === f;
export const matchSearch = (ex, q) => !q || `${ex.name} ${ex.description || ''}`.toLowerCase().includes(q);

export const MUSCLE_LABELS = {
  chest: 'Грудь', back: 'Спина', shoulders: 'Плечи', biceps: 'Бицепс',
  triceps: 'Трицепс', legs: 'Ноги', core: 'Кор', full_body: 'Всё тело',
};
export const TYPE_LABELS = {
  strength: 'Силовое', cardio: 'Кардио', stretch: 'Растяжка', bodyweight: 'С весом тела',
};
export const MUSCLE_COLORS = {
  chest: 'blue', back: 'purple', shoulders: 'orange', biceps: 'red',
  triceps: 'red', legs: 'green', core: 'yellow', full_body: 'gray',
};
export const TYPE_COLORS = {
  strength: 'blue', cardio: 'red', stretch: 'green', bodyweight: 'purple',
};
