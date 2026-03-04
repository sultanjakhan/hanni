// ── db-view/db-view.js — DatabaseView orchestrator class ──

import { S, invoke } from '../state.js';
import { renderToolbar } from './db-toolbar.js';
import { renderTableView } from './db-table.js';
import { renderKanbanView } from './db-kanban.js';
import { renderListView } from './db-list.js';
import { renderGalleryView } from './db-gallery.js';

/**
 * DatabaseView — Notion-style multi-view database component.
 *
 * Usage:
 *   const dbv = new DatabaseView(containerEl, schema);
 *   await dbv.render();
 *
 * Schema options: see db-config.js for full shape.
 */
export class DatabaseView {
  /**
   * @param {HTMLElement} el - Container element to render into
   * @param {object} schema - Configuration (see db-config.js)
   */
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
  }

  /** Main render entry point */
  async render() {
    const s = this.schema;

    // Fetch records
    if (s.records) {
      this._records = s.records;
    } else if (s.fetchRecords) {
      try { this._records = await s.fetchRecords(); } catch { this._records = []; }
    }

    // Load custom props
    try { this._customProps = await invoke('get_property_definitions', { tabId: s.tabId }); } catch { this._customProps = []; }

    // Load property values
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

  /** Render the currently selected view into the content element */
  async _renderView(contentEl) {
    const s = this.schema;
    const ctx = {
      tabId: s.tabId,
      recordTable: s.recordTable,
      records: this._records,
      fixedColumns: s.fixedColumns || [],
      idField: s.idField,
      customProps: this._customProps,
      valuesMap: this._valuesMap,
      reloadFn: s.reloadFn || (() => this.render()),
      onRowClick: s.onRowClick,
      onAdd: s.onAdd,
      addButton: s.addButton,
      kanban: s.kanban,
      gallery: s.gallery,
      onSort: (sortKey, dir) => this._handleSort(sortKey, dir),
      onDrop: s.onDrop,
    };

    switch (this._currentView) {
      case 'kanban':
        renderKanbanView(contentEl, ctx);
        break;
      case 'list':
        renderListView(contentEl, ctx);
        break;
      case 'gallery':
        renderGalleryView(contentEl, ctx);
        break;
      case 'table':
      default:
        await renderTableView(contentEl, ctx);
        break;
    }
  }

  /** Handle sort from table view */
  _handleSort(sortKey, dir) {
    const s = this.schema;
    const sorted = [...this._records].sort((a, b) => {
      let va, vb;
      if (sortKey.startsWith('prop_')) {
        const pid = parseInt(sortKey.substring(5));
        va = this._valuesMap[a[s.idField]]?.[pid] ?? '';
        vb = this._valuesMap[b[s.idField]]?.[pid] ?? '';
      } else {
        va = a[sortKey] ?? '';
        vb = b[sortKey] ?? '';
      }
      if (typeof va === 'number' && typeof vb === 'number') return dir === 'asc' ? va - vb : vb - va;
      return dir === 'asc' ? String(va).localeCompare(String(vb)) : String(vb).localeCompare(String(va));
    });
    this._records = sorted;
    this.render();
  }

  /** Load persisted view type from view_configs */
  async _loadPersistedView() {
    try {
      const configs = await invoke('get_view_configs', { tabId: this.schema.tabId });
      if (configs.length > 0 && configs[0].view_type) {
        const vt = configs[0].view_type;
        if (this.schema.availableViews.includes(vt)) return vt;
      }
    } catch {}
    return null;
  }

  /** Persist view type to view_configs */
  async _persistView(viewType) {
    try {
      const configs = await invoke('get_view_configs', { tabId: this.schema.tabId });
      if (configs.length > 0) {
        await invoke('update_view_config', { id: configs[0].id, filterJson: null, sortJson: null, visibleColumns: null });
        // update_view_config doesn't have viewType param, so we use a workaround: delete + recreate
        // Actually, let's just store in localStorage for now since the Rust command may not support viewType update
      } else {
        await invoke('create_view_config', { tabId: this.schema.tabId, name: 'Default', viewType });
      }
    } catch {}
    // Also store in localStorage as fallback
    localStorage.setItem(`dbv_view_${this.schema.tabId}`, viewType);
  }

  /** Load persisted view with localStorage fallback */
  async _loadPersistedViewFull() {
    // Try DB first
    const dbView = await this._loadPersistedView();
    if (dbView) return dbView;
    // Fallback to localStorage
    const lsView = localStorage.getItem(`dbv_view_${this.schema.tabId}`);
    if (lsView && this.schema.availableViews.includes(lsView)) return lsView;
    return null;
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
