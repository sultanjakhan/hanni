// ── db-view/db-type-registry.js — Centralized type definitions ──

const F_TEXT = ['contains', 'not_contains', 'eq', 'neq', 'starts_with', 'ends_with', 'empty', 'not_empty'];
const F_NUM = ['eq', 'neq', 'gt', 'lt', 'empty', 'not_empty'];
const F_DATE = ['eq', 'before', 'after', 'this_week', 'this_month', 'last_7_days', 'last_30_days', 'empty', 'not_empty'];
const F_TIME = ['eq', 'before', 'after', 'empty', 'not_empty'];
const F_CHOICE = ['eq', 'neq', 'contains', 'empty', 'not_empty'];
const F_BOOL = ['eq', 'empty', 'not_empty'];

export const TYPE_REGISTRY = {
  text:         { id: 'text',         icon: 'Aa', name: 'Текст',        editor: 'text',     filters: F_TEXT },
  number:       { id: 'number',       icon: '#',  name: 'Число',        editor: 'number',   filters: F_NUM,
                  validate: v => v === '' || !isNaN(parseFloat(v)) },
  select:       { id: 'select',       icon: '◉', name: 'Выбор',        editor: 'select',   filters: F_CHOICE },
  multi_select: { id: 'multi_select', icon: '☰', name: 'Мульти-выбор', editor: 'multi_select', filters: F_CHOICE },
  date:         { id: 'date',         icon: '◫', name: 'Дата',         editor: 'date',     filters: F_DATE },
  time:         { id: 'time',         icon: '⏰', name: 'Время',       editor: 'time',     filters: F_TIME,
                  validate: v => v === '' || /^([01]\d|2[0-3]):[0-5]\d$/.test(v) },
  checkbox:     { id: 'checkbox',     icon: '☑', name: 'Чекбокс',      editor: 'checkbox', filters: F_BOOL },
  url:          { id: 'url',          icon: '↗', name: 'Ссылка',       editor: 'text',     filters: F_TEXT },
  status:       { id: 'status',       icon: '◔', name: 'Статус',       editor: 'select',   filters: F_CHOICE },
  email:        { id: 'email',        icon: '@',  name: 'Email',        editor: 'text',     filters: F_TEXT },
  phone:        { id: 'phone',        icon: '☎', name: 'Телефон',      editor: 'text',     filters: F_TEXT },
  progress:     { id: 'progress',     icon: '◐', name: 'Прогресс',     editor: 'progress', filters: F_NUM,
                  validate: v => { const n = parseInt(v); return v === '' || (n >= 0 && n <= 100); } },
  rating:       { id: 'rating',       icon: '★', name: 'Рейтинг',      editor: 'rating',   filters: F_NUM,
                  validate: v => { const n = parseInt(v); return v === '' || (n >= 1 && n <= 5); } },
  created_time: { id: 'created_time', icon: '⏱', name: 'Создано',      editor: 'readonly', filters: F_DATE, auto: true },
  last_edited:  { id: 'last_edited',  icon: '✎', name: 'Изменено',     editor: 'readonly', filters: F_DATE, auto: true },
  unique_id:    { id: 'unique_id',    icon: '#',  name: 'ID',           editor: 'readonly', filters: F_NUM,  auto: true },
};

/** Get type definition by id */
export function getType(typeId) {
  return TYPE_REGISTRY[typeId] || TYPE_REGISTRY.text;
}

/** Get type icon */
export function getTypeIcon(typeId) {
  return (TYPE_REGISTRY[typeId] || TYPE_REGISTRY.text).icon;
}

/** Get type display name */
export function getTypeName(typeId) {
  return (TYPE_REGISTRY[typeId] || TYPE_REGISTRY.text).name;
}

/** Get type list for UI (add property popover, type changer) */
export function getTypeList() {
  return Object.values(TYPE_REGISTRY);
}

/** Get filter condition IDs for a type */
export function getFilterConditions(typeId) {
  return (TYPE_REGISTRY[typeId] || TYPE_REGISTRY.text).filters;
}
