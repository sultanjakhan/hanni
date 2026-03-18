import { S, invoke } from '../state.js';
import { renderToolbar } from './db-toolbar.js';
import { renderTableView } from './db-table.js';
import { renderKanbanView } from './db-kanban.js';
import { renderListView } from './db-list.js';
import { renderGalleryView } from './db-gallery.js';

export class DatabaseView {
  constructor(el, schema) {
    this.el = el;
    this.schema = {
      idField: 'id',
      availableViews: ['table'],
      defaultView: 'table',
      ...schema,
    };
    this._currentView = null;
    this._records = [];
    this._customProps = [];
    this._valuesMap = {};
    this._sortRules = []; // [{key, dir}] — multi-level sort
  }

  /** Main render entry point */
  async render() {
    const s = this.schema;

    this._records = s.records || (s.fetchRecords ? await s.fetchRecords().catch(() => []) : []);
    try { this._customProps = await invoke('get_property_definitions', { tabId: s.tabId }); } catch { this._customProps = []; }
    const recordIds = this._records.map(r => r[s.idField]);
    if (recordIds.length > 0 && this._customProps.length > 0) {
      try {
        const allValues = await invoke('get_property_values', { recordTable: s.recordTable, recordIds });
        this._valuesMap = {};
        for (const v of allValues) {
          if (!this._valuesMap[v.record_id]) this._valuesMap[v.record_id] = {};
          this._valuesMap[v.record_id][v.property_id] = v.value;
        }
      } catch { this._valuesMap = {}; }
    }

    // Resolve current view type (persisted or default)
    if (!this._currentView) {
      this._currentView = await this._loadPersistedViewFull() || s.defaultView;
    }

    // Clear & render
    this.el.innerHTML = '';

    // Wrapper div for content (toolbar prepends before it)
    const contentEl = document.createElement('div');
    contentEl.className = 'dbv-content';
    this.el.appendChild(contentEl);

    // Toolbar
    renderToolbar(this.el, s.availableViews, this._currentView, (vt) => this._switchView(vt));

    // Render current view
    await this._renderView(contentEl);
  }

  /** Switch to a different view type */
  async _switchView(viewType) {
    this._currentView = viewType;
    this._persistView(viewType);
    await this.render();
  }

  async _renderView(contentEl) {
    const s = this.schema;
    const ctx = {
      tabId: s.tabId, recordTable: s.recordTable, records: this._records,
      fixedColumns: s.fixedColumns || [], idField: s.idField, customProps: this._customProps,
      valuesMap: this._valuesMap, reloadFn: s.reloadFn || (() => this.render()),
      onRowClick: s.onRowClick, onAdd: s.onAdd, addButton: s.addButton,
      kanban: s.kanban, gallery: s.gallery, onDrop: s.onDrop,
      onSort: (key, dir, multi) => this._handleSort(key, dir, multi), sortRules: this._sortRules,
    };
    const renderers = { kanban: renderKanbanView, list: renderListView, gallery: renderGalleryView };
    const fn = renderers[this._currentView] || renderTableView;
    await fn(contentEl, ctx);
  }

  /** Handle sort — multi=true adds secondary sort (Shift+click) */
  _handleSort(sortKey, dir, multi = false) {
    if (multi) {
      const idx = this._sortRules.findIndex(r => r.key === sortKey);
      if (idx >= 0) this._sortRules[idx].dir = dir;
      else this._sortRules.push({ key: sortKey, dir });
    } else {
      this._sortRules = [{ key: sortKey, dir }];
    }
    this._applySortRules();
    this.render();
  }

  _applySortRules() {
    if (this._sortRules.length === 0) return;
    const s = this.schema;
    const vm = this._valuesMap;
    const getVal = (rec, key) => {
      if (key.startsWith('prop_')) return vm[rec[s.idField]]?.[parseInt(key.substring(5))] ?? '';
      return rec[key] ?? '';
    };
    this._records.sort((a, b) => {
      for (const { key, dir } of this._sortRules) {
        const va = getVal(a, key), vb = getVal(b, key);
        let cmp = typeof va === 'number' && typeof vb === 'number' ? va - vb : String(va).localeCompare(String(vb));
        if (cmp !== 0) return dir === 'asc' ? cmp : -cmp;
      }
      return 0;
    });
  }

  async _persistView(viewType) {
    try {
      const configs = await invoke('get_view_configs', { tabId: this.schema.tabId });
      if (configs.length === 0) await invoke('create_view_config', { tabId: this.schema.tabId, name: 'Default', viewType });
    } catch {}
    localStorage.setItem(`dbv_view_${this.schema.tabId}`, viewType);
  }

  async _loadPersistedViewFull() {
    try {
      const configs = await invoke('get_view_configs', { tabId: this.schema.tabId });
      if (configs[0]?.view_type && this.schema.availableViews.includes(configs[0].view_type)) return configs[0].view_type;
    } catch {}
    const ls = localStorage.getItem(`dbv_view_${this.schema.tabId}`);
    return ls && this.schema.availableViews.includes(ls) ? ls : null;
  }
}

// Re-export everything for convenience
export { registerSchema, getSchema, getSchemaIds } from './db-config.js';
export { renderTableView } from './db-table.js';
export { renderKanbanView } from './db-kanban.js';
export { renderListView } from './db-list.js';
export { renderGalleryView } from './db-gallery.js';
export { formatPropValue, startInlineEdit } from './db-cell-editors.js';
export { renderFilterBar, applyFilters, showFilterBuilderModal, saveFiltersToViewConfig, loadFiltersFromViewConfig } from './db-filters.js';
export { showAddPropertyModal, showColumnMenu } from './db-properties.js';
export { renderToolbar } from './db-toolbar.js';
