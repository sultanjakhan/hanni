// ── db-view/db-config.js — Tab schema definitions registry ──

/**
 * Schema registry: maps schema IDs to config objects.
 *
 * Schema shape:
 * {
 *   tabId: string,
 *   recordTable: string,
 *   fixedColumns: [{ key, label, render? }],
 *   idField?: string (default 'id'),
 *   availableViews?: string[] (default ['table']),
 *   defaultView?: string (default 'table'),
 *   kanban?: { groupByField, columns: [{ key, label, icon?, color? }] },
 *   gallery?: { renderCard?, minCardWidth? },
 *   fetchRecords: () => Promise<array>,
 *   onAdd?: () => void,
 *   onRowClick?: (record) => void,
 *   addButton?: string,
 *   reloadFn?: () => void,
 *   onDrop?: (recordId, field, newValue) => void,
 * }
 */
const _schemas = {};

/** Register a database view schema */
export function registerSchema(id, schema) {
  _schemas[id] = { idField: 'id', availableViews: ['table'], defaultView: 'table', ...schema };
}

/** Get a registered schema by ID */
export function getSchema(id) {
  return _schemas[id] || null;
}

/** Get all registered schema IDs */
export function getSchemaIds() {
  return Object.keys(_schemas);
}

/** Remove a schema */
export function unregisterSchema(id) {
  delete _schemas[id];
}
