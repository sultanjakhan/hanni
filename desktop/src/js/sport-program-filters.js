// ── sport-program-filters.js — Filter logic for workout programs ──

export const PROGRAM_KINDS = [
  { id: 'all', label: 'Все' }, { id: 'monthly', label: 'Месячная' },
  { id: 'split', label: 'Сплит' }, { id: 'muscle_focus', label: 'На группу' },
  { id: 'warmup', label: 'Разминка' }, { id: 'custom', label: 'Своя' },
];

export const KIND_LABELS = {
  monthly: 'Месячная', split: 'Сплит', muscle_focus: 'На группу',
  warmup: 'Разминка', custom: 'Своя',
};

export const KIND_COLORS = {
  monthly: 'purple', split: 'blue', muscle_focus: 'orange', warmup: 'green', custom: 'gray',
};

export const matchKind = (p, f) => f === 'all' || p.kind === f;
export const matchSearch = (p, q) => !q || `${p.name} ${p.notes || ''}`.toLowerCase().includes(q);
