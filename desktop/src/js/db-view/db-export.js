// ── db-view/db-export.js — Export table data to CSV ──

/** Export filtered records + custom properties to CSV and trigger download */
export function exportToCsv(records, fixedColumns, visibleProps, valuesMap, idField, tabId) {
  const headers = [];
  fixedColumns.forEach(c => headers.push(c.label));
  visibleProps.forEach(p => headers.push(p.name));

  const rows = [headers];
  for (const record of records) {
    const row = [];
    fixedColumns.forEach(c => {
      row.push(csvEscape(String(record[c.key] ?? '')));
    });
    visibleProps.forEach(p => {
      const val = valuesMap[record[idField]]?.[p.id] ?? '';
      row.push(csvEscape(String(val)));
    });
    rows.push(row);
  }

  const csv = rows.map(r => r.join(',')).join('\n');
  const blob = new Blob(['\uFEFF' + csv], { type: 'text/csv;charset=utf-8;' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${tabId}_export_${new Date().toISOString().slice(0, 10)}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

function csvEscape(val) {
  if (val.includes(',') || val.includes('"') || val.includes('\n')) {
    return '"' + val.replace(/"/g, '""') + '"';
  }
  return val;
}
