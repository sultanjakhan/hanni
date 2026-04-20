// ── db-view/db-view.js — DatabaseView orchestrator class ──

import { S, invoke } from '../state.js';
import { renderToolbar } from './db-toolbar.js';
import { renderTableView } from './db-table.js';
import { renderKanbanView } from './db-kanban.js';
import { renderListView } from './db-list.js';
import { renderGalleryView } from './db-gallery.js';
import { renderTimelineView } from './db-timeline.js';
import { renderCalendarView } from './db-calendar.js';
import { showFilterDropdown, renderFilterBar } from './db-filters.js';
import { showSortDropdown, getSortRules, applySortRules } from './db-sort.js';
import { exportToCsv } from './db-export.js';
import { importCsv } from './db-import.js';
import { getHiddenFixedCols, setHiddenFixedCols, getDeletedFixedCols } from './db-col-state.js';

export class DatabaseView {
  constructor(el, schema) {
    this.el = el;
    this.schema = { idField: 'id', availableViews: ['table'], defaultView: 'table', ...schema };
    Object.assign(this, { _currentView: null, _records: [], _customProps: [], _valuesMap: {}, _searchQuery: '' });
    const removed = JSON.parse(localStorage.getItem(`dbv_removed_${this.schema.tabId}`) || '[]');
    this.schema.availableViews = this.schema.availableViews.filter(v => !removed.includes(v));
    if (!this.schema.availableViews.includes('table')) this.schema.availableViews.unshift('table');
  }

  async render() {
    const s = this.schema;
    this._records = s.records || (s.fetchRecords ? await s.fetchRecords().catch(() => []) : []);
    try { this._customProps = await invoke('get_property_definitions', { tabId: s.tabId }); } catch { this._customProps = []; }
    await this._loadValues();
    if (!this._currentView) this._currentView = await this._loadPersistedView() || s.defaultView;

    // Preserve scroll position and height across full re-renders
    const oldWrap = this.el.querySelector('.dbv-table-wrap');
    const savedScroll = oldWrap ? { top: oldWrap.scrollTop, left: oldWrap.scrollLeft } : null;
    const savedHeight = this.el.offsetHeight;
    if (savedHeight > 0) this.el.style.minHeight = savedHeight + 'px';

    this.el.innerHTML = '';
    const contentEl = document.createElement('div');
    contentEl.className = 'dbv-content';
    this.el.appendChild(contentEl);
    const allFields = this._buildFields();
    this._renderToolbar(allFields, contentEl);
    await this._renderView(contentEl, allFields);

    // Restore scroll position and release height lock
    if (savedScroll) {
      const newWrap = this.el.querySelector('.dbv-table-wrap');
      if (newWrap) { newWrap.scrollTop = savedScroll.top; newWrap.scrollLeft = savedScroll.left; }
    }
    this.el.style.minHeight = '';
  }

  _renderToolbar(allFields, contentEl) {
    const s = this.schema, visProp = () => this._customProps.filter(p => p.visible !== false);
    const hiddenCols = this._getHiddenColumns();
    if (!S._dbvQuickFilter) S._dbvQuickFilter = {};
    renderToolbar(this.el, s.availableViews, this._currentView, (vt) => this._switchView(vt), {
      onSearch: (q) => { this._searchQuery = q; this._renderView(contentEl, allFields); },
      onFilter: (a) => showFilterDropdown(a, s.tabId, allFields, () => this.render()),
      onQuickFilter: (mode) => { S._dbvQuickFilter[s.tabId] = mode; this.render(); },
      quickFilter: S._dbvQuickFilter[s.tabId] || null,
      onSort: (a) => showSortDropdown(a, s.tabId, this._customProps, s.fixedColumns || [], () => this._handleSort()),
      onAdd: s.onQuickAdd ? () => { this._searchQuery = ''; s.onQuickAdd(); } : null,
      onExport: () => exportToCsv(this._applySearch(this._records), s.fixedColumns || [], visProp(), this._valuesMap, s.idField, s.tabId),
      onImport: () => importCsv(s.tabId, s.fixedColumns || [], this._customProps, s.reloadFn || (() => this.render())),
      onDeleteView: (vt) => this._removeView(vt),
      onAddView: (vt) => this._addView(vt),
      hiddenColumns: hiddenCols,
      onShowColumn: (col) => this._showColumn(col),
      onFrozenView: s.onFrozenView,
      frozenCount: s.frozenCount,
    });
  }

  async _renderView(contentEl, allFields) {
    const s = this.schema, records = this._applySearch(this._records);
    const ctx = {
      tabId: s.tabId, recordTable: s.recordTable, records,
      fixedColumns: s.fixedColumns || [], idField: s.idField,
      customProps: this._customProps, valuesMap: this._valuesMap,
      reloadFn: s.reloadFn || (() => this.render()),
      onRowClick: s.onRowClick, onAdd: s.onAdd, onQuickAdd: s.onQuickAdd,
      addButton: s.addButton, kanban: s.kanban, gallery: s.gallery,
      onSort: () => this._handleSort(),
      onDrop: s.onDrop, onCellEdit: s.onCellEdit, onDelete: s.onDelete, onDuplicate: s.onDuplicate, onFreeze: s.onFreeze,
    };
    const views = { kanban: renderKanbanView, list: renderListView, gallery: renderGalleryView, timeline: renderTimelineView, calendar: renderCalendarView };
    const fn = views[this._currentView];
    if (fn) fn(contentEl, ctx); else await renderTableView(contentEl, ctx);
    renderFilterBar(contentEl, s.tabId, allFields || this._buildFields(), () => this.render());
  }

  _applySearch(records) {
    if (!this._searchQuery) return records;
    const q = this._searchQuery.toLowerCase(), s = this.schema;
    return records.filter(r => {
      for (const c of (s.fixedColumns || [])) if (String(r[c.key] ?? '').toLowerCase().includes(q)) return true;
      const v = this._valuesMap[r[s.idField]]; return v ? Object.values(v).some(x => String(x ?? '').toLowerCase().includes(q)) : false;
    });
  }
  _buildFields() {
    const s = this.schema;
    return [...(s.fixedColumns || []).map(c => ({ filterKey: c.key, label: c.label, type: c.editType || 'text', options: c.editOptions || [] })),
      ...this._customProps.filter(p => p.visible !== false).map(p => {
        let opts = []; try { opts = JSON.parse(p.options || '[]'); } catch {} return { filterKey: `prop_${p.id}`, label: p.name, type: p.type, options: opts };
      })];
  }
  async _loadValues() {
    const s = this.schema, ids = this._records.map(r => r[s.idField]);
    if (!ids.length || !this._customProps.length) return;
    try { const all = await invoke('get_property_values', { recordTable: s.recordTable, recordIds: ids }); this._valuesMap = {};
      for (const v of all) { if (!this._valuesMap[v.record_id]) this._valuesMap[v.record_id] = {}; this._valuesMap[v.record_id][v.property_id] = v.value; }
    } catch { this._valuesMap = {}; }
  }
  _handleSort(key, dir) {
    const tabId = this.schema.tabId;
    if (key && dir) {
      if (!S._dbvSortRules) S._dbvSortRules = {};
      const rules = S._dbvSortRules[tabId] || [];
      const idx = rules.findIndex(r => r.key === key);
      if (idx >= 0) rules[idx].dir = dir; else rules.push({ key, dir });
      S._dbvSortRules[tabId] = rules;
    }
    const r = getSortRules(tabId);
    applySortRules(this._records, r, this.schema.idField, this._valuesMap);
    this.render();
  }
  _getHiddenColumns() {
    const s = this.schema, result = [], hiddenFixed = getHiddenFixedCols(s.tabId), deletedFixed = getDeletedFixedCols(s.tabId);
    for (const key of hiddenFixed) { if (deletedFixed.includes(key)) continue; const col = (s.fixedColumns || []).find(c => c.key === key); result.push({ id: key, name: col ? col.label : key, kind: 'fixed' }); }
    for (const p of this._customProps) { if (p.visible === false) result.push({ id: p.id, name: p.name, kind: 'custom' }); }
    return result;
  }
  async _showColumn(col) {
    const s = this.schema, reload = s.reloadFn || (() => this.render());
    if (col.kind === 'fixed') { const h = getHiddenFixedCols(s.tabId); await setHiddenFixedCols(s.tabId, h.filter(k => k !== col.id)); }
    else await invoke('update_property_definition', { id: col.id, name: null, propType: null, position: null, color: null, options: null, visible: true });
    reload();
  }

  async _switchView(vt) { this._currentView = vt; this._persistView(vt); await this.render(); }
  _addView(vt) {
    const s = this.schema; if (!s.availableViews.includes(vt)) s.availableViews.push(vt);
    const rm = JSON.parse(localStorage.getItem(`dbv_removed_${s.tabId}`) || '[]'), idx = rm.indexOf(vt);
    if (idx !== -1) { rm.splice(idx, 1); localStorage.setItem(`dbv_removed_${s.tabId}`, JSON.stringify(rm)); }
    this._switchView(vt);
  }
  _removeView(vt) {
    const s = this.schema, vi = s.availableViews.indexOf(vt); if (vi === -1) return;
    s.availableViews.splice(vi, 1); if (this._currentView === vt) this._currentView = 'table';
    this._persistView(this._currentView);
    const rm = JSON.parse(localStorage.getItem(`dbv_removed_${s.tabId}`) || '[]');
    if (!rm.includes(vt)) { rm.push(vt); localStorage.setItem(`dbv_removed_${s.tabId}`, JSON.stringify(rm)); }
    this.render();
  }

  async _loadPersistedView() {
    try { const c = await invoke('get_view_configs', { tabId: this.schema.tabId }); if (c[0]?.view_type && this.schema.availableViews.includes(c[0].view_type)) return c[0].view_type; } catch {}
    const ls = localStorage.getItem(`dbv_view_${this.schema.tabId}`);
    return ls && this.schema.availableViews.includes(ls) ? ls : null;
  }
  async _persistView(vt) {
    try { const c = await invoke('get_view_configs', { tabId: this.schema.tabId }); if (c.length > 0) await invoke('update_view_config', { id: c[0].id, viewType: vt, filterJson: null, sortJson: null, visibleColumns: null }); else await invoke('create_view_config', { tabId: this.schema.tabId, name: 'Default', viewType: vt }); } catch {}
    localStorage.setItem(`dbv_view_${this.schema.tabId}`, vt);
  }
}

export { registerSchema, getSchema, getSchemaIds } from './db-config.js'; export { renderTableView } from './db-table.js';
export { renderKanbanView } from './db-kanban.js'; export { renderListView } from './db-list.js';
export { renderGalleryView } from './db-gallery.js'; export { renderTimelineView } from './db-timeline.js';
export { renderCalendarView } from './db-calendar.js'; export { formatPropValue, startInlineEdit } from './db-cell-editors.js';
export { renderFilterBar, applyFilters, showFilterDropdown } from './db-filters.js'; export { renderToolbar } from './db-toolbar.js';
export { showAddPropertyPopover, showColumnMenu, showFixedColumnMenu, getHiddenFixedCols, getDeletedFixedCols, getFixedColName, getColumnOrder, setColumnOrder } from './db-properties.js';
export { exportToCsv } from './db-export.js'; export { importCsv } from './db-import.js';
