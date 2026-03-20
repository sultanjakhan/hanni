// ── tab-data-custom.js — Custom Pages tab ──

import { S, invoke, tabLoaders, TAB_REGISTRY, COMMON_EMOJIS } from './state.js';
import { escapeHtml, confirmModal, initBlockEditor, blocksToPlainText, migrateTextToBlocks } from './utils.js';
import { renderTabBar, closeTab } from './tabs.js';

// ── Custom Pages ──
export async function loadCustomPage(tabId) {
  const reg = TAB_REGISTRY[tabId];
  if (!reg?.custom || !reg.pageId) return;
  const el = document.getElementById(`${tabId}-content`);
  if (!el) return;

  try {
    const page = await invoke('get_custom_page', { id: reg.pageId });

    el.innerHTML = `
      <div class="custom-page-header">
        <div class="custom-page-icon-row">
          <button class="custom-page-icon-btn" id="cp-icon-btn" title="Сменить иконку">${escapeHtml(page.icon || '📄')}</button>
          <button class="btn-danger btn-small custom-page-delete-btn" id="cp-delete-btn">Удалить</button>
        </div>
        <input class="page-title-input" id="cp-title" value="${escapeHtml(page.title || '')}" placeholder="Без названия">
        <input class="page-description-input" id="cp-desc" value="${escapeHtml(page.description || '')}" placeholder="Добавить описание...">
      </div>
      <div class="custom-page-content">
        <div id="cp-editor" class="block-editor-container"></div>
      </div>
      <div class="custom-page-emoji-picker hidden" id="cp-emoji-picker">
        ${COMMON_EMOJIS.map(e => `<button class="emoji-pick-btn">${e}</button>`).join('')}
      </div>`;

    // Auto-save helper for metadata fields
    const autoSaveMeta = (field, value) => {
      clearTimeout(S.customPageAutoSave);
      S.customPageAutoSave = setTimeout(async () => {
        const args = { id: reg.pageId };
        args[field] = value;
        await invoke('update_custom_page', args).catch(() => {});
        if (field === 'title') { reg.label = value || 'Без названия'; renderTabBar(); }
        if (field === 'icon') { reg.icon = value; renderTabBar(); }
      }, 500);
    };

    // Auto-save for Editor.js content
    const autoSaveContent = async () => {
      clearTimeout(S.customPageAutoSave);
      S.customPageAutoSave = setTimeout(async () => {
        if (!S.currentCpEditor) return;
        try {
          const output = await S.currentCpEditor.save();
          const contentBlocks = JSON.stringify(output);
          const content = blocksToPlainText(output);
          await invoke('update_custom_page', { id: reg.pageId, content, contentBlocks }).catch(() => {});
        } catch (e) { console.error('cp editor save error:', e); }
      }, 500);
    };

    document.getElementById('cp-title')?.addEventListener('input', (e) => autoSaveMeta('title', e.target.value));
    document.getElementById('cp-desc')?.addEventListener('input', (e) => autoSaveMeta('description', e.target.value));

    // Initialize Editor.js for custom page
    let editorData = null;
    if (page.content_blocks) {
      try { editorData = JSON.parse(page.content_blocks); } catch (e) { console.error('parse cp content_blocks:', e); }
    }
    if (!editorData && page.content) {
      editorData = migrateTextToBlocks(page.content);
    }

    // Destroy previous editor instance
    if (S.currentCpEditor) {
      try { S.currentCpEditor.destroy(); } catch (e) {}
      S.currentCpEditor = null;
    }
    S.currentCpEditor = initBlockEditor('cp-editor', editorData, () => autoSaveContent());

    // Emoji picker
    const emojiPicker = document.getElementById('cp-emoji-picker');
    document.getElementById('cp-icon-btn')?.addEventListener('click', (e) => {
      e.stopPropagation();
      emojiPicker?.classList.toggle('hidden');
    });
    document.querySelectorAll('.emoji-pick-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const emoji = btn.textContent;
        document.getElementById('cp-icon-btn').textContent = emoji;
        emojiPicker?.classList.add('hidden');
        autoSaveMeta('icon', emoji);
      });
    });
    // Close emoji picker on outside click
    const closeEmojiPicker = (e) => {
      if (!emojiPicker?.contains(e.target) && e.target.id !== 'cp-icon-btn') {
        emojiPicker?.classList.add('hidden');
      }
    };
    document.addEventListener('click', closeEmojiPicker);

    // Delete page
    document.getElementById('cp-delete-btn')?.addEventListener('click', async () => {
      if (!(await confirmModal('Удалить страницу?'))) return;
      if (S.currentCpEditor) {
        try { S.currentCpEditor.destroy(); } catch (e) {}
        S.currentCpEditor = null;
      }
      await invoke('delete_custom_page', { id: reg.pageId }).catch(() => {});
      closeTab(tabId);
      delete TAB_REGISTRY[tabId];
      const viewDiv = document.getElementById(`view-${tabId}`);
      if (viewDiv) viewDiv.remove();
    });

  } catch (e) {
    el.innerHTML = `<div class="empty-state"><div class="empty-state-icon">📄</div><div class="empty-state-text">Страница не найдена</div></div>`;
  }
}
