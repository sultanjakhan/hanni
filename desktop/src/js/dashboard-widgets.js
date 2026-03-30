// dashboard-widgets.js — Renderers for each widget type
import { escapeHtml } from './utils.js';

export function extractPath(obj, path) {
  if (!path || !obj) return obj;
  if (path === '_count') return Array.isArray(obj) ? obj.length : 0;
  if (path === '_length') return Array.isArray(obj) ? obj.length : typeof obj === 'string' ? obj.length : 0;
  return path.split('.').reduce((o, k) => {
    if (o == null) return undefined;
    if (k === '_count') return Array.isArray(o) ? o.length : 0;
    if (k.startsWith('_filter:')) {
      const [, field, val] = k.match(/^_filter:(\w+)=(.+)$/) || [];
      return Array.isArray(o) ? o.filter(item => String(item[field]) === val).length : 0;
    }
    return o[k];
  }, obj);
}

export function renderStatWidget(el, widget, data) {
  const c = widget.config;
  const val = extractPath(data, c.valuePath) ?? c.emptyValue ?? '—';
  const suffix = c.suffix || '';
  el.className = `uni-dash-card dash-color-${c.color || 'blue'}`;
  el.innerHTML = `
    <div class="uni-dash-value">${escapeHtml(String(val))}${escapeHtml(suffix)}</div>
    <div class="uni-dash-label">${escapeHtml(c.label || '')}</div>`;
}

export function renderInteractiveWidget(el, widget, data, reloadFn) {
  const c = widget.config;
  const val = extractPath(data, c.valuePath);
  const display = val != null ? `${val}${c.suffix || ''}` : (c.emptyValue || '—');
  el.className = `uni-dash-card dash-color-${c.color || 'blue'} dash-interactive`;
  el.innerHTML = `
    <div class="uni-dash-value">${escapeHtml(String(display))}</div>
    <div class="uni-dash-label">${escapeHtml(c.label || '')}</div>`;
  el.addEventListener('click', async () => {
    const input = prompt(c.action?.prompt || c.label);
    if (input == null) return;
    const args = { ...(c.action?.commandArgs || {}) };
    const param = c.action?.valueParam || 'value';
    args[param] = c.action?.valueType === 'float' ? parseFloat(input) : input;
    try {
      const { invoke } = await import('./state.js');
      await invoke(c.action.command, args);
      reloadFn();
    } catch (e) { alert(e); }
  });
}

export function renderProgressWidget(el, widget, data) {
  const c = widget.config;
  const val = Number(extractPath(data, c.valuePath) ?? 0);
  const target = c.target || 100;
  const pct = Math.min(100, Math.round((val / target) * 100));
  el.className = `uni-dash-card dash-color-${c.color || 'green'}`;
  el.innerHTML = `
    <div class="uni-dash-value">${val}${escapeHtml(c.suffix || '')} <span class="dash-target">/ ${target}</span></div>
    <div class="uni-dash-label">${escapeHtml(c.label || '')}</div>
    <div class="dash-progress-bar"><div class="dash-progress-fill" style="width:${pct}%"></div></div>`;
}

export function renderListWidget(el, widget, data) {
  const c = widget.config;
  const items = Array.isArray(data) ? data.slice(0, c.limit || 5) : [];
  const rows = items.map(r => {
    const title = extractPath(r, c.titlePath) || '—';
    const sub = c.subtitlePath ? extractPath(r, c.subtitlePath) || '' : '';
    return `<div class="dash-list-item"><span>${escapeHtml(String(title))}</span><span class="text-muted">${escapeHtml(String(sub))}</span></div>`;
  }).join('');
  el.className = `uni-dash-card dash-color-${c.color || 'blue'} dash-list-card`;
  el.innerHTML = `
    <div class="uni-dash-label" style="margin-bottom:8px">${escapeHtml(c.label || '')}</div>
    ${rows || '<div class="text-muted">Пусто</div>'}`;
}

export function renderTextWidget(el, widget) {
  const c = widget.config;
  el.className = `uni-dash-card dash-color-${c.color || 'blue'}`;
  el.innerHTML = `
    <div class="uni-dash-label">${escapeHtml(c.label || '')}</div>
    <div style="margin-top:4px;font-size:13px">${escapeHtml(c.content || '')}</div>`;
}
