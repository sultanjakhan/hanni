import { S } from '../state.js';

/**
 * Group records by a property value.
 * Returns [{label, records, collapsed}] or null if no grouping active.
 */
export function groupRecords(records, valuesMap, idField, tabId, customProps) {
  const groupPropId = S.dbvGroupBy?.[tabId];
  if (!groupPropId) return null;

  const prop = customProps.find(p => p.id === groupPropId);
  if (!prop) return null;

  const groups = new Map();
  const noGroup = [];

  const isDate = prop.type === 'date';
  for (const r of records) {
    const rid = r[idField];
    const raw = valuesMap[rid]?.[groupPropId] ?? '';
    if (!raw) { noGroup.push(r); continue; }
    const val = isDate ? raw.substring(0, 7) : raw; // group dates by YYYY-MM
    if (!groups.has(val)) groups.set(val, []);
    groups.get(val).push(r);
  }

  const collapsed = S.dbvGroupCollapsed?.[tabId] || {};
  const result = [];
  for (const [label, recs] of groups) {
    result.push({ label, records: recs, collapsed: !!collapsed[label] });
  }
  const hideEmpty = S._dbvHideEmptyGroups?.[tabId];
  if (noGroup.length > 0 && !hideEmpty) {
    result.push({ label: 'Без группы', records: noGroup, collapsed: !!collapsed['Без группы'] });
  }
  return result;
}

/** Toggle collapsed state for a group */
export function toggleGroupCollapse(tabId, label) {
  if (!S.dbvGroupCollapsed) S.dbvGroupCollapsed = {};
  if (!S.dbvGroupCollapsed[tabId]) S.dbvGroupCollapsed[tabId] = {};
  S.dbvGroupCollapsed[tabId][label] = !S.dbvGroupCollapsed[tabId][label];
}

/** Render group by selector in toolbar area */
export function renderGroupBySelector(container, tabId, customProps, reloadFn) {
  const groupable = customProps.filter(p => ['select', 'status', 'date'].includes(p.type));
  if (groupable.length === 0) return;

  const current = S.dbvGroupBy?.[tabId];
  const currentProp = groupable.find(p => p.id === current);
  const label = currentProp ? currentProp.name : 'Group by';

  const btn = document.createElement('button');
  btn.className = 'btn-secondary dbv-group-btn';
  btn.textContent = currentProp ? `⊞ ${label}` : '⊞ Group';
  container.querySelector('.dbv-filter-bar')?.appendChild(btn);

  btn.addEventListener('click', () => {
    const menu = document.createElement('div');
    menu.className = 'col-context-menu';
    const hideEmpty = S._dbvHideEmptyGroups?.[tabId];
    menu.innerHTML = `<div class="col-menu-item" data-gid="none">Без группировки</div>` +
      groupable.map(p => `<div class="col-menu-item${p.id === current ? ' active' : ''}" data-gid="${p.id}">${p.name}</div>`).join('') +
      (current ? `<div style="border-top:1px solid var(--border-subtle);margin:4px 0;"></div><div class="col-menu-item" data-gid="hide-empty">${hideEmpty ? '☑' : '☐'} Скрыть пустые</div>` : '');
    const rect = btn.getBoundingClientRect();
    menu.style.left = rect.left + 'px';
    menu.style.top = rect.bottom + 4 + 'px';
    document.body.appendChild(menu);
    menu.querySelectorAll('.col-menu-item').forEach(item => {
      item.addEventListener('click', () => {
        const gid = item.dataset.gid;
        if (gid === 'hide-empty') {
          if (!S._dbvHideEmptyGroups) S._dbvHideEmptyGroups = {};
          S._dbvHideEmptyGroups[tabId] = !S._dbvHideEmptyGroups[tabId];
        } else {
          if (!S.dbvGroupBy) S.dbvGroupBy = {};
          S.dbvGroupBy[tabId] = gid === 'none' ? null : parseInt(gid);
        }
        menu.remove();
        if (reloadFn) reloadFn();
      });
    });
    setTimeout(() => document.addEventListener('mousedown', (e) => { if (!menu.contains(e.target)) menu.remove(); }, { once: true }), 10);
  });
}

/** Render grouped table body HTML with collapsible headers */
export function renderGroupedBody(groups, fixedColumns, visibleProps, idField, valuesMap, escapeHtml, formatPropValue, getTypeIcon, tabId) {
  let html = '';
  const colspan = fixedColumns.length + visibleProps.length + 1;

  for (const g of groups) {
    const arrow = g.collapsed ? '▶' : '▼';
    html += `<tr class="group-header-row" data-group="${escapeHtml(g.label)}"><td colspan="${colspan}"><span class="group-toggle">${arrow}</span> <strong>${escapeHtml(g.label)}</strong> <span class="text-faint">(${g.records.length})</span></td></tr>`;
    if (!g.collapsed) {
      for (const record of g.records) {
        const rid = record[idField];
        const tdFixed = fixedColumns.map(c => {
          const val = c.render ? c.render(record) : escapeHtml(String(record[c.key] ?? ''));
          return `<td>${val}</td>`;
        }).join('');
        const tdCustom = visibleProps.map(p => {
          const autoVals = { created_time: record.created_at, last_edited: record.updated_at, unique_id: rid };
          const rawVal = autoVals[p.type] ?? valuesMap[rid]?.[p.id] ?? '';
          const displayVal = formatPropValue(rawVal, p);
          return `<td class="cell-editable" data-record-id="${rid}" data-prop-id="${p.id}" data-prop-type="${p.type}" data-prop-options='${escapeHtml(p.options || "[]")}'>${displayVal}</td>`;
        }).join('');
        html += `<tr class="data-table-row" data-id="${rid}">${tdFixed}${tdCustom}<td></td></tr>`;
      }
    }
  }
  return html;
}
