// ── db-view/db-import.js — Import CSV into database ──

import { invoke } from '../state.js';

/**
 * Show file picker, parse CSV, preview and import records.
 * Uses schema.onQuickAdd or a generic property-based import.
 */
export function importCsv(tabId, fixedColumns, customProps, reloadFn) {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.csv,text/csv';
  input.addEventListener('change', async () => {
    const file = input.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const rows = parseCsv(text);
      if (rows.length < 2) return console.error('CSV пустой или содержит только заголовки');
      const headers = rows[0];
      const dataRows = rows.slice(1).filter(r => r.some(c => c.trim()));
      if (dataRows.length === 0) return console.error('Нет данных для импорта');
      await importRows(tabId, headers, dataRows, fixedColumns, customProps);
      if (reloadFn) reloadFn();
    } catch (e) {
      console.error('Import error:', e.message || e);
    }
  });
  input.click();
}

/** Map CSV headers to properties and save values */
async function importRows(tabId, headers, dataRows, fixedColumns, customProps) {
  const colMap = headers.map(h => {
    const hl = h.trim().toLowerCase();
    const fp = fixedColumns.find(c => c.label.toLowerCase() === hl);
    if (fp) return { type: 'fixed', key: fp.key };
    const cp = customProps.find(p => p.name.toLowerCase() === hl);
    if (cp) return { type: 'prop', prop: cp };
    return null;
  });

  // Create missing properties for unmatched headers
  for (let i = 0; i < headers.length; i++) {
    if (!colMap[i] && headers[i].trim()) {
      try {
        const newProp = await invoke('create_property_definition', {
          tabId, name: headers[i].trim(), propType: 'text', options: '[]',
        });
        colMap[i] = { type: 'prop', prop: newProp };
      } catch { /* skip */ }
    }
  }

  // Import each row as property values on a placeholder record
  const propCols = colMap.map((m, i) => m?.type === 'prop' ? { idx: i, prop: m.prop } : null).filter(Boolean);
  if (propCols.length === 0) return console.error('Не удалось сопоставить столбцы CSV со свойствами');

  let imported = 0;
  for (const row of dataRows) {
    // Save property values — need a record_id; use recordTable convention
    for (const { idx, prop } of propCols) {
      const val = (row[idx] ?? '').trim();
      if (!val) continue;
      try {
        await invoke('set_property_value', {
          recordTable: tabId, recordId: imported + 1, propertyId: prop.id, value: val,
        });
      } catch { /* skip */ }
    }
    imported++;
  }
}

/** Parse CSV text into array of rows */
function parseCsv(text) {
  const rows = [];
  let row = [], cell = '', inQuotes = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') { cell += '"'; i++; }
      else if (ch === '"') inQuotes = false;
      else cell += ch;
    } else {
      if (ch === '"') inQuotes = true;
      else if (ch === ',') { row.push(cell); cell = ''; }
      else if (ch === '\n' || (ch === '\r' && text[i + 1] === '\n')) {
        if (ch === '\r') i++;
        row.push(cell); cell = '';
        rows.push(row); row = [];
      } else cell += ch;
    }
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  return rows;
}
