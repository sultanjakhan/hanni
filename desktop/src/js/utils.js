// ── js/utils.js — Shared utilities, Markdown, skeletons, helpers ──

import { invoke, S, TAB_REGISTRY, TAB_SETTINGS_DEFS, PAGE_EMOJIS, saveTabCustom, getTabDesc, loadTabSetting, saveTabSetting, tabLoaders } from './state.js';

// ── Markdown rendering setup ──
const markedInstance = new marked.Marked({
  breaks: true,
  gfm: true,
  highlight: (code, lang) => {
    if (lang && hljs.getLanguage(lang)) {
      return hljs.highlight(code, { language: lang }).value;
    }
    return hljs.highlightAuto(code).value;
  },
});
markedInstance.use({
  renderer: {
    code({ text, lang }) {
      const highlighted = lang && hljs.getLanguage(lang)
        ? hljs.highlight(text, { language: lang }).value
        : hljs.highlightAuto(text).value;
      const langLabel = lang || 'code';
      return `<div class="code-block"><div class="code-header"><span>${langLabel}</span><button class="code-copy-btn" onclick="navigator.clipboard.writeText(this.closest('.code-block').querySelector('code').textContent)">Копировать</button></div><pre><code class="hljs">${highlighted}</code></pre></div>`;
    },
    link({ href, text }) {
      return `<a href="#" onclick="event.preventDefault();window.__TAURI__.core.invoke('open_url',{url:'${href.replace(/'/g, "\\'")}'});return false;">${text}</a>`;
    },
  },
});

export function renderMarkdown(text) {
  return markedInstance.parse(text);
}

export function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML.replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

// ── Skeleton loaders ──

export function skeletonSettings(rows = 3) {
  let html = '<div class="skeleton-card">';
  html += '<div class="skeleton skeleton-header"></div>';
  for (let i = 0; i < rows; i++) {
    html += `<div class="skeleton-row"><div class="skeleton skeleton-line w-1-4"></div><div class="skeleton skeleton-line w-1-4"></div></div>`;
  }
  html += '</div>';
  return html;
}

export function skeletonGrid(cols = 4) {
  let html = '<div style="display:grid;grid-template-columns:repeat(' + cols + ',1fr);gap:10px;margin-bottom:20px;">';
  for (let i = 0; i < cols; i++) {
    html += '<div class="skeleton-stat"><div class="skeleton skeleton-line w-1-2" style="margin:0 auto 6px;height:20px;"></div><div class="skeleton skeleton-line w-3-4" style="margin:0 auto;height:10px;"></div></div>';
  }
  html += '</div>';
  return html;
}

export function skeletonList(items = 5) {
  let html = '';
  for (let i = 0; i < items; i++) {
    const w = i % 3 === 0 ? 'w-3-4' : i % 3 === 1 ? 'w-full' : 'w-1-2';
    html += `<div class="skeleton skeleton-line ${w}"></div>`;
  }
  return html;
}

export function skeletonPage() {
  return skeletonGrid(4) + skeletonSettings(3) + skeletonSettings(2);
}

// ── Confirm modal ──

export function confirmModal(msg = 'Удалить?', confirmLabel = 'Да') {
  return new Promise(resolve => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal modal-compact" style="max-width:320px;text-align:center;">
      <div class="modal-title">${escapeHtml(msg)}</div>
      <div class="modal-actions">
        <button class="btn-secondary confirm-no">Отмена</button>
        <button class="btn-primary confirm-yes" style="background:var(--color-red)">${escapeHtml(confirmLabel)}</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    overlay.querySelector('.confirm-no').onclick = () => { overlay.remove(); resolve(false); };
    overlay.querySelector('.confirm-yes').onclick = () => { overlay.remove(); resolve(true); };
    overlay.addEventListener('click', e => { if (e.target === overlay) { overlay.remove(); resolve(false); } });
  });
}

// ── History message helpers ──

export function normalizeHistoryMessage(msg) {
  if (Array.isArray(msg)) {
    return { role: msg[0], content: msg[1] };
  }
  if (msg.proactive) {
    const { proactive, ...rest } = msg;
    return rest;
  }
  return msg;
}

export function getRole(msg) {
  if (Array.isArray(msg)) return msg[0];
  return msg.role;
}

export function getContent(msg) {
  if (Array.isArray(msg)) return msg[1];
  return msg.content || '';
}

// ── Page header ──

export function renderPageHeader(tabId, extra) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg) return '';
  const customIcon = S.tabCustomizations[tabId]?.icon;
  const iconHtml = customIcon
    ? `<button class="page-header-icon-btn" data-tab-id="${tabId}" title="Сменить иконку">${customIcon}</button>`
    : `<button class="page-header-icon-btn page-header-icon-svg" data-tab-id="${tabId}" title="Сменить иконку">${reg.icon || ''}</button>`;
  const desc = extra?.description || getTabDesc(tabId);
  const props = extra?.properties || [];
  return `<div class="page-header" data-tab-id="${tabId}">
    ${iconHtml}
    <div class="page-header-title">${extra?.title || reg.label}</div>
    <input class="page-header-desc-input" data-tab-id="${tabId}" value="${escapeHtml(desc)}" placeholder="Добавить описание...">
    ${props.length ? `<div class="page-header-properties">${props.map(p =>
      `<span class="page-property"><span class="page-property-label">${p.label}</span><span class="page-property-value ${p.class || ''}">${p.value}</span></span>`
    ).join('')}</div>` : ''}
    <div class="page-emoji-picker hidden" id="page-emoji-picker-${tabId}">
      ${PAGE_EMOJIS.map(e => `<button class="emoji-pick-btn" data-emoji="${e}">${e}</button>`).join('')}
    </div>
  </div>`;
}

export function setupPageHeaderControls(tabId) {
  const iconBtn = document.querySelector(`.page-header-icon-btn[data-tab-id="${tabId}"]`);
  const picker = document.getElementById(`page-emoji-picker-${tabId}`);
  if (iconBtn && picker) {
    iconBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      picker.classList.toggle('hidden');
    });
    picker.querySelectorAll('.emoji-pick-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const emoji = btn.dataset.emoji;
        if (!S.tabCustomizations[tabId]) S.tabCustomizations[tabId] = {};
        S.tabCustomizations[tabId].icon = emoji;
        saveTabCustom();
        iconBtn.textContent = emoji;
        iconBtn.classList.remove('page-header-icon-svg');
        picker.classList.add('hidden');
        // Late-bound: renderTabBar
        tabLoaders._renderTabBar?.();
      });
    });
    const closePicker = (e) => {
      if (!picker.contains(e.target) && e.target !== iconBtn) picker.classList.add('hidden');
    };
    document.addEventListener('click', closePicker, { once: false });
  }

  const descInput = document.querySelector(`.page-header-desc-input[data-tab-id="${tabId}"]`);
  if (descInput) {
    descInput.addEventListener('input', () => {
      if (!S.tabCustomizations[tabId]) S.tabCustomizations[tabId] = {};
      S.tabCustomizations[tabId].desc = descInput.value;
      saveTabCustom();
    });
  }
}

// ── Tab settings ──
// loadTabSetting / saveTabSetting are imported from state.js (canonical location)

export async function renderTabSettingsPage(tabId) {
  const reg = TAB_REGISTRY[tabId];
  const el = document.getElementById(`${tabId}-content`);
  if (!el || !reg) return;
  const defs = TAB_SETTINGS_DEFS[tabId] || [];

  let rowsHtml = '';
  for (const def of defs) {
    const val = await loadTabSetting(tabId, def.key) ?? def.default;
    let controlHtml = '';
    if (def.type === 'toggle') {
      controlHtml = `<label class="toggle"><input type="checkbox" data-tab-id="${tabId}" data-setting-key="${def.key}" ${val === 'true' ? 'checked' : ''}><span class="toggle-track"></span></label>`;
    } else if (def.type === 'select') {
      controlHtml = `<div class="setting-pills" data-tab-id="${tabId}" data-setting-key="${def.key}">` +
        def.options.map(o => `<button class="setting-pill${val === o.value ? ' active' : ''}" data-value="${o.value}">${o.label}</button>`).join('') + `</div>`;
    } else if (def.type === 'number') {
      controlHtml = `<input class="form-input" type="number" min="${def.min || 1}" max="${def.max || 480}" step="1" data-tab-id="${tabId}" data-setting-key="${def.key}" value="${escapeHtml(val)}" style="width:100px;">`;
    } else {
      controlHtml = `<input class="form-input" type="text" data-tab-id="${tabId}" data-setting-key="${def.key}" value="${escapeHtml(val)}">`;
    }
    rowsHtml += `<div class="settings-row"><span class="settings-label">${def.label}</span><span class="settings-value">${controlHtml}</span></div>`;
  }

  el.innerHTML = renderPageHeader(tabId) + `<div class="page-content">
    <div class="settings-section">
      <div class="settings-section-title">Настройки — ${reg.label}</div>
      ${rowsHtml || '<div style="color:var(--text-muted);font-size:13px;">Нет настроек для этой вкладки</div>'}
    </div>
  </div>`;
  setupPageHeaderControls(tabId);

  el.querySelectorAll('input[data-setting-key], select[data-setting-key]').forEach(ctrl => {
    ctrl.addEventListener('change', () => {
      const v = ctrl.type === 'checkbox' ? ctrl.checked : ctrl.value;
      saveTabSetting(ctrl.dataset.tabId, ctrl.dataset.settingKey, v);
    });
  });

  el.querySelectorAll('.setting-pills').forEach(group => {
    group.querySelectorAll('.setting-pill').forEach(pill => {
      pill.addEventListener('click', () => {
        group.querySelectorAll('.setting-pill').forEach(p => p.classList.remove('active'));
        pill.classList.add('active');
        saveTabSetting(group.dataset.tabId, group.dataset.settingKey, pill.dataset.value);
      });
    });
  });
}

// ── Editor.js block editor helpers ──

export function initBlockEditor(holderId, data, onChange, opts = {}) {
  const { customTools = {}, readOnly = false, placeholder } = opts;
  const baseTools = {
    header: { class: Header, config: { levels: [1, 2, 3], defaultLevel: 2 } },
    list: { class: EditorjsList, inlineToolbar: true },
    checklist: { class: Checklist, inlineToolbar: true },
    quote: { class: Quote },
    code: CodeTool,
    delimiter: Delimiter,
    marker: { class: Marker },
    inlineCode: { class: InlineCode },
  };
  const editor = new EditorJS({
    holder: holderId,
    data: data || { blocks: [] },
    placeholder: placeholder || 'Нажмите / для команд или начните писать...',
    readOnly,
    tools: { ...baseTools, ...customTools },
    onChange: async (api) => {
      if (readOnly) return;
      const output = await api.saver.save();
      if (onChange) onChange(output);
    },
    onReady: () => {
      if (!readOnly && window.DragDrop) new DragDrop(editor);
    },
  });
  return editor;
}

// ── Tab block editor (shared for all tabs) ──

let _tabBlockSaveTimeouts = {};

export async function loadTabBlockEditor(tabId, subTab, contentEl, defaultBlocks) {
  const key = `${tabId}::${subTab || ''}`;
  const holderId = `tab-block-${tabId}-${(subTab || 'main').replace(/\s+/g, '-')}`;

  // Create holder div
  let holderEl = contentEl.querySelector(`#${holderId}`);
  if (!holderEl) {
    holderEl = document.createElement('div');
    holderEl.id = holderId;
    holderEl.className = 'tab-block-editor';
    contentEl.appendChild(holderEl);
  } else {
    holderEl.innerHTML = '';
  }

  // Load saved blocks from DB
  let data = null;
  try {
    const json = await invoke('get_tab_blocks', { tabId, subTab: subTab || '' });
    if (json) data = JSON.parse(json);
  } catch (_) {}

  if (!data) data = defaultBlocks || { blocks: [{ type: 'paragraph', data: { text: '' } }] };

  // Init editor with auto-save (500ms debounce)
  const editor = initBlockEditor(holderId, data, (output) => {
    clearTimeout(_tabBlockSaveTimeouts[key]);
    _tabBlockSaveTimeouts[key] = setTimeout(() => {
      invoke('save_tab_blocks', {
        tabId,
        subTab: subTab || '',
        blocksJson: JSON.stringify(output),
      }).catch(() => {});
    }, 500);
  }, { placeholder: 'Нажмите / для команд или начните писать...' });

  return editor;
}

export function blocksToPlainText(data) {
  return (data?.blocks || []).map(b => {
    if (b.type === 'checklist') return (b.data.items || []).map(i => i.text).join('\n');
    if (b.type === 'list') {
      const extractItems = (items) => (items || []).map(i => typeof i === 'string' ? i : i.content || i.text || '').join('\n');
      return extractItems(b.data.items);
    }
    return b.data?.text || '';
  }).join('\n');
}

export function migrateTextToBlocks(text) {
  if (!text) return { blocks: [] };
  return {
    blocks: text.split('\n\n').filter(Boolean).map(p => ({
      type: 'paragraph',
      data: { text: p.replace(/\n/g, '<br>') }
    }))
  };
}
