// ── js/chat.js — Chat messages, sending, streaming, input chips, file attachment, drag-drop, proactive/typing listeners, welcome card, chat settings ──

import { S, invoke, listen, emit, chat, input, sendBtn, attachBtn, fileInput, attachPreview, tabLoaders, TAB_REGISTRY, TAB_ICONS, PROACTIVE_STYLE_DEFINITIONS, MEMORY_CATEGORIES, VOICE_SERVER, setTheme } from './state.js';
import { renderMarkdown, escapeHtml, normalizeHistoryMessage, getRole, getContent, confirmModal, skeletonPage, renderPageHeader } from './utils.js';
import { autoSaveConversation, loadConversationsList } from './conversations.js';
import { showChatSettingsMode, hideChatSettingsMode } from './tabs.js';

// ── Auto-update notification ──
listen('update-available', (event) => {
  const version = event.payload;
  const banner = document.createElement('div');
  banner.style.cssText = 'padding:8px 16px;background:var(--bg-card);color:var(--text-secondary);font-size:12px;text-align:center;border-bottom:1px solid var(--border-default);';
  banner.textContent = `Обновление до v${version}...`;
  document.getElementById('content-area')?.prepend(banner);
});

// ── Proactive message listener ──
listen('proactive-message', async (event) => {
  // Prevent race condition: don't mutate history while chat is streaming
  if (S.busy) return;
  // v0.22: payload is now {text, id} JSON
  const payload = typeof event.payload === 'object' ? event.payload : { text: event.payload, id: 0 };
  const text = payload.text;
  const proactiveId = payload.id || 0;
  S.lastProactiveTime = Date.now();

  // Use addMsg to get proper wrapper with TTS button
  removeChatWelcomeCard();
  const msgDiv = addMsg('bot', text);
  const wrapper = msgDiv.closest('.msg-wrapper');
  if (wrapper) {
    msgDiv.classList.add('proactive');
  }

  const ts = document.createElement('div');
  ts.className = 'proactive-time';
  ts.textContent = new Date().toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  chat.appendChild(ts);

  scrollDown();

  // Save to proactive_messages table for sidebar
  invoke('save_proactive_message', { text }).then(() => {
    tabLoaders.loadConversationsList?.();
  }).catch(() => {});

  // Add proactive feedback buttons (copy + thumbs — no regen)
  if (wrapper) {
    addProactiveFeedbackButtons(wrapper, proactiveId, text);
  }

  // Desktop notification if window not focused
  if (!document.hasFocus()) {
    new Notification('Hanni', { body: text });
  }
});

// ── Auto-quiet status listener ──
listen('proactive-auto-quiet', (event) => {
  const badge = document.getElementById('proactive-status-badge');
  if (badge) {
    if (event.payload) {
      badge.textContent = 'Тишина (авто)';
      badge.classList.add('quiet');
    } else {
      badge.textContent = 'Активен';
      badge.classList.remove('quiet');
    }
  }
});

// ── Typing signal ──
input.addEventListener('input', () => {
  invoke('set_user_typing', { typing: true }).catch(() => {});
  clearTimeout(S.typingTimeout);
  S.typingTimeout = setTimeout(() => {
    invoke('set_user_typing', { typing: false }).catch(() => {});
  }, 10000);
});

// ── Reminder notifications ──
listen('reminder-fired', (event) => {
  const title = event.payload;
  addMsg('bot', `\u23F0 Напоминание: ${title}`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Напоминание', { body: title });
  }
});

listen('note-reminder-fired', (event) => {
  const { id, title } = event.payload;
  addMsg('bot', `\u{1F4DD} Заметка-напоминание: **${title}**`);
  scrollDown();
  if (!document.hasFocus()) {
    new Notification('Заметка', { body: title });
  }
});

// ── Focus mode listener ──

listen('focus-ended', () => {
  addMsg('bot', 'Фокус-режим завершён!');
  if (S.focusTimerInterval) {
    clearInterval(S.focusTimerInterval);
    S.focusTimerInterval = null;
  }
  if (tabLoaders.updateFocusWidget) tabLoaders.updateFocusWidget();
});

// ── Chat Settings ──

async function loadChatSettings() {
  const el = document.getElementById('chat-settings-content');
  if (!el) return;
  el.innerHTML = skeletonPage();
  try {
    const [proactive, ttsVoices, ttsServerUrl, memories, voiceCloneEnabled, voiceCloneSample, voiceSamples, trainStats, trainFlywheel, trainHistory, useOpenClaw] = await Promise.all([
      invoke('get_proactive_settings').catch(() => ({ enabled: false, interval_minutes: 15, active_hours_start: 9, active_hours_end: 23, reply_window_sec: 120, styles: [] })),
      invoke('get_tts_voices').catch(() => []),
      invoke('get_app_setting', { key: 'tts_server_url' }).catch(() => null),
      invoke('get_all_memories', { search: null }).catch(() => []),
      invoke('get_app_setting', { key: 'voice_clone_enabled' }).catch(() => null),
      invoke('get_app_setting', { key: 'voice_clone_sample' }).catch(() => null),
      invoke('list_voice_samples').catch(() => []),
      invoke('get_training_stats').catch(() => ({ conversations: 0, total_messages: 0 })),
      invoke('get_flywheel_status').catch(() => ({ thumbs_up_total: 0, new_pairs: 0, total_cycles: 0, ready_to_train: false })),
      invoke('get_flywheel_history').catch(() => []),
      invoke('get_app_setting', { key: 'use_openclaw' }).catch(() => null),
    ]);
    const voicesByLang = {};
    for (const v of ttsVoices) {
      const lang = v.lang || 'other';
      if (!voicesByLang[lang]) voicesByLang[lang] = [];
      voicesByLang[lang].push(v);
    }
    const langOrder = ['ru-RU', 'kk-KZ', 'en-US'];
    const sortedLangs = [...langOrder.filter(l => voicesByLang[l]), ...Object.keys(voicesByLang).filter(l => !langOrder.includes(l)).sort()];

    const enabledStyles = proactive.enabled_styles || PROACTIVE_STYLE_DEFINITIONS.map(s => s.id);
    const quietStart = proactive.quiet_start_time || `${String(proactive.quiet_hours_start ?? 23).padStart(2,'0')}:00`;
    const quietEnd = proactive.quiet_end_time || `${String(proactive.quiet_hours_end ?? 8).padStart(2,'0')}:00`;

    el.innerHTML = `
      <div class="chat-settings-tabs">
        <button class="chat-settings-tab active" data-panel="memory">Память</button>
        <button class="chat-settings-tab" data-panel="general">Автономный</button>
        <button class="chat-settings-tab" data-panel="voice">Голос</button>
        <button class="chat-settings-tab" data-panel="styles">Стили</button>
        <button class="chat-settings-tab" data-panel="tools">Инструменты</button>
        <button class="chat-settings-tab" data-panel="data">Данные</button>
        <button class="chat-settings-tab" data-panel="appearance">Оформление</button>
        <button class="chat-settings-tab" data-panel="about">О Hanni</button>
      </div>

      <div class="chat-settings-panel active" id="cs-panel-memory">
        <div class="memory-header">
          <div class="memory-search-box" style="flex:1;">
            <input class="form-input" id="cs-mem-search" placeholder="Поиск по памяти..." autocomplete="off">
          </div>
          <button class="btn-primary" id="cs-mem-add-btn">+ Добавить</button>
        </div>
        <div style="color:var(--text-muted);font-size:12px;margin-bottom:12px;" id="cs-mem-count">${memories.length} фактов</div>
        <div class="memory-browser" id="cs-mem-list"></div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-general">
        <div class="settings-section">
          <div class="settings-section-title">Автономный режим <span class="proactive-status-badge" id="proactive-status-badge">Активен</span></div>
          <div class="settings-row">
            <span class="settings-label">Включён</span>
            <label class="toggle">
              <input type="checkbox" id="chat-proactive-enabled" ${proactive.enabled ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Интервал</span>
            <div class="proactive-interval-row">
              <input type="range" id="chat-proactive-slider" class="proactive-slider" min="1" max="60" value="${proactive.interval_minutes}">
              <input type="number" id="chat-proactive-number" class="proactive-interval-number" min="1" max="120" value="${proactive.interval_minutes}">
              <span class="proactive-interval-unit">мин</span>
            </div>
          </div>
          <div class="settings-row">
            <span class="settings-label">Тихие часы</span>
            <div class="quiet-hours-row">
              <span class="quiet-hours-label">С</span>
              <input type="time" id="chat-quiet-start" class="form-input" style="width:90px;text-align:center" value="${quietStart}">
              <span class="quiet-hours-separator">—</span>
              <span class="quiet-hours-label">До</span>
              <input type="time" id="chat-quiet-end" class="form-input" style="width:90px;text-align:center" value="${quietEnd}">
            </div>
          </div>
          <div class="settings-row">
            <span class="settings-label">Лимит в день</span>
            <div class="proactive-interval-row">
              <input type="number" id="chat-daily-limit" class="proactive-interval-number" min="0" max="100" value="${proactive.daily_limit ?? 20}">
              <span class="proactive-interval-unit">сообщ. (0 = безлимит)</span>
            </div>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-voice">
        <div class="settings-section">
          <div class="settings-section-title">Озвучка</div>
          <div class="settings-row">
            <span class="settings-label">Включена</span>
            <label class="toggle">
              <input type="checkbox" id="chat-voice-enabled" ${proactive.voice_enabled ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Голос</span>
            <select class="form-select" id="chat-voice-name" style="width:260px">
              ${sortedLangs.map(lang => `
                <optgroup label="${lang}">
                  ${(voicesByLang[lang] || []).map(v =>
                    `<option value="${v.name}" ${proactive.voice_name === v.name ? 'selected' : ''}>${v.name} (${v.gender})</option>`
                  ).join('')}
                </optgroup>
              `).join('')}
            </select>
          </div>
          <div class="settings-row">
            <span class="settings-label"></span>
            <button class="settings-btn" id="chat-test-voice">Прослушать</button>
          </div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">TTS Сервер (PC)</div>
          <div class="settings-row">
            <span class="settings-label">URL сервера</span>
            <input class="form-input" id="chat-tts-server-url" placeholder="http://192.168.x.x:8236" value="${ttsServerUrl || ''}" style="width:220px">
          </div>
          <div class="settings-row">
            <span class="settings-label">Статус</span>
            <span class="settings-value" id="chat-tts-server-status">—</span>
          </div>
          <div class="settings-row">
            <span class="settings-label"></span>
            <button class="settings-btn" id="chat-tts-server-save">Сохранить</button>
          </div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Клонирование голоса (PC)</div>
          <div class="settings-row">
            <span class="settings-label">Использовать клон</span>
            <label class="toggle">
              <input type="checkbox" id="chat-voice-clone-enabled" ${voiceCloneEnabled === 'true' ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row">
            <span class="settings-label">Образец голоса</span>
            <select class="form-select" id="chat-voice-clone-sample" style="width:220px">
              <option value="">— нет —</option>
              ${voiceSamples.map(s => `<option value="${s.name}" ${voiceCloneSample === s.name ? 'selected' : ''}>${s.name}</option>`).join('')}
            </select>
          </div>
          <div class="settings-row">
            <span class="settings-label"></span>
            <button class="settings-btn" id="chat-record-voice-sample">Записать образец</button>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-styles">
        <div class="settings-section">
          <div class="settings-section-title">Стили проактивных сообщений</div>
          <div class="proactive-styles-section">
            <div class="proactive-styles-desc">Какие стили Hanni может использовать в автономном режиме.</div>
            <div class="proactive-styles-actions">
              <button class="btn-smallall" id="proactive-select-all">Все</button>
              <button class="btn-smallall" id="proactive-select-none">Снять все</button>
            </div>
            <div class="proactive-styles-grid" id="proactive-styles-grid">
              ${PROACTIVE_STYLE_DEFINITIONS.map(s => {
                const isEnabled = enabledStyles.includes(s.id);
                return `<div class="proactive-style-card${isEnabled ? ' enabled' : ''}" data-style-id="${s.id}">
                  <span class="proactive-style-icon">${s.icon}</span>
                  <div class="proactive-style-info">
                    <div class="proactive-style-name">${s.name}</div>
                    <div class="proactive-style-desc">${s.desc}</div>
                  </div>
                  <label class="mini-toggle proactive-style-toggle">
                    <input type="checkbox" ${isEnabled ? 'checked' : ''} data-style="${s.id}">
                    <span class="mini-toggle-slider"></span>
                  </label>
                </div>`;
              }).join('')}
            </div>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-tools">
        <div class="settings-section">
          <div class="settings-section-title">AI движок</div>
          <div class="settings-row">
            <span class="settings-label">OpenClaw Gateway</span>
            <label class="toggle">
              <input type="checkbox" id="chat-use-openclaw" ${useOpenClaw === 'true' ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
          <div class="settings-row" style="opacity:0.7">
            <span class="settings-label" style="font-size:12px">Агент, 24 инструмента, Talk Mode, Telegram</span>
          </div>
          <div class="settings-row" style="opacity:0.7">
            <span class="settings-label" style="font-size:12px">Дашборд: <a href="#" onclick="window.__TAURI__.shell.open('http://127.0.0.1:18789/');return false" style="color:var(--blue)">localhost:18789</a></span>
          </div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Встроенные инструменты</div>
          <div class="settings-row" style="opacity:0.7">
            <span class="settings-label" style="font-size:12px">web_search, read_url, set_timer, open_app, media, calendar, notes, money</span>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-data">
        <div class="settings-section">
          <div class="settings-section-title">Данные для обучения</div>
          <div class="settings-row"><span class="settings-label">Диалогов (4+ сообщений)</span><span class="settings-value">${trainStats.conversations}</span></div>
          <div class="settings-row"><span class="settings-label">Всего сообщений</span><span class="settings-value">${trainStats.total_messages}</span></div>
          <div class="settings-row"><span class="settings-label">Thumbs-up пар</span><span class="settings-value" id="train-pairs-count">...</span></div>
          <div class="settings-row"><span class="settings-label">Период</span><span class="settings-value">${trainStats.earliest ? trainStats.earliest.substring(0,10) + ' — ' + trainStats.latest.substring(0,10) : '—'}</span></div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Экспорт</div>
          <div class="settings-row"><span class="settings-label">JSONL</span><button class="settings-btn" id="train-export-btn">Экспорт данных</button></div>
        </div>
        <div class="settings-section">
          <div class="settings-section-title">Data Flywheel</div>
          <div class="settings-row"><span class="settings-label">Всего thumbs-up</span><span class="settings-value">${trainFlywheel.thumbs_up_total}</span></div>
          <div class="settings-row"><span class="settings-label">Новых (не экспортировано)</span><span class="settings-value" style="${trainFlywheel.new_pairs >= 20 ? 'color:var(--accent)' : ''}">${trainFlywheel.new_pairs}</span></div>
          <div class="settings-row"><span class="settings-label">Циклов обучения</span><span class="settings-value">${trainFlywheel.total_cycles}</span></div>
          <div class="settings-row"><span class="settings-label">Последний цикл</span><span class="settings-value">${trainFlywheel.last_cycle ? trainFlywheel.last_cycle.date.substring(0,10) + ' — ' + trainFlywheel.last_cycle.status : '—'}</span></div>
          <div class="settings-row">
            <span class="settings-label">Полный цикл</span>
            <button class="settings-btn ${trainFlywheel.ready_to_train ? 'btn-primary' : ''}" id="flywheel-run-btn" ${trainFlywheel.ready_to_train ? '' : 'title="Нужно минимум 20 новых пар"'}>
              ${trainFlywheel.ready_to_train ? 'Запустить цикл' : 'Мало данных (' + trainFlywheel.new_pairs + '/20)'}
            </button>
          </div>
          <div id="flywheel-progress" class="hidden" style="margin-top:8px;">
            <div class="train-progress-bar"><div class="train-progress-fill" id="flywheel-fill"></div></div>
            <div id="flywheel-status" style="font-size:12px;color:var(--text-secondary);margin-top:4px;"></div>
          </div>
          ${trainHistory.length > 0 ? `
          <div style="margin-top:12px;">
            <div style="font-size:12px;color:var(--text-secondary);margin-bottom:6px;">История циклов:</div>
            ${trainHistory.slice(0, 5).map(c => `
              <div style="font-size:12px;color:var(--text-secondary);padding:3px 0;border-bottom:1px solid var(--border-color);">
                #${c.id} ${(c.started_at || '').substring(0,16)} — ${c.status} (${c.train_pairs} пар)${c.eval_score != null ? ' score:' + c.eval_score.toFixed(2) : ''}
              </div>
            `).join('')}
          </div>` : ''}
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-appearance">
        <div class="settings-section">
          <div class="settings-section-title">Тема оформления</div>
          <div class="settings-row">
            <div>
              <span class="settings-label">Тёмная тема</span>
            </div>
            <label class="toggle">
              <input type="checkbox" id="cs-theme-toggle" ${S.theme === 'dark' ? 'checked' : ''}>
              <span class="toggle-slider"></span>
            </label>
          </div>
        </div>
      </div>

      <div class="chat-settings-panel" id="cs-panel-about">
        <div class="about-wrapper">
          <div class="about-card">
            <div class="about-header">
              <div class="about-logo">\u{1F916}</div>
              <div class="about-name">Hanni</div>
              <span class="about-version">v${S.APP_VERSION}</span>
            </div>
            <hr class="about-divider">
            <div class="about-info-list">
              <div class="about-info-row"><span class="about-info-label">Модель</span><span class="about-info-value" id="cs-about-model">Загрузка...</span></div>
              <div class="about-info-row"><span class="about-info-label">MLX сервер</span><span class="about-info-value" id="cs-about-mlx">Загрузка...</span></div>
              <div class="about-info-row"><span class="about-info-label">HTTP API</span><span class="about-info-value" id="cs-about-api">Проверяю...</span></div>
            </div>
            <hr class="about-divider">
            <div class="about-actions">
              <button class="settings-btn" id="cs-about-check-update">Проверить обновления</button>
            </div>
          </div>
        </div>
      </div>`;

    // ── Memory panel ──
    _csMemRenderList(memories);
    _chatSettingsSetupMemory(memories);

    // ── Tab switching ──
    document.querySelectorAll('.chat-settings-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.chat-settings-tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.chat-settings-panel').forEach(p => p.classList.remove('active'));
        tab.classList.add('active');
        document.getElementById(`cs-panel-${tab.dataset.panel}`)?.classList.add('active');
      });
    });

    // ── Save handlers ──
    const getEnabledStyles = () => {
      const checks = document.querySelectorAll('#proactive-styles-grid input[data-style]');
      return Array.from(checks).filter(c => c.checked).map(c => c.dataset.style);
    };
    const getChatProactiveValues = () => {
      const qStart = document.getElementById('chat-quiet-start')?.value || '23:00';
      const qEnd = document.getElementById('chat-quiet-end')?.value || '08:00';
      const [sh, sm] = qStart.split(':').map(Number);
      const [eh, em] = qEnd.split(':').map(Number);
      return {
        enabled: document.getElementById('chat-proactive-enabled').checked,
        voice_enabled: document.getElementById('chat-voice-enabled').checked,
        voice_name: document.getElementById('chat-voice-name')?.value || 'xenia',
        interval_minutes: parseInt(document.getElementById('chat-proactive-number')?.value || '10'),
        quiet_hours_start: sh,
        quiet_hours_end: eh,
        quiet_start_time: qStart,
        quiet_end_time: qEnd,
        enabled_styles: getEnabledStyles(),
        daily_limit: parseInt(document.getElementById('chat-daily-limit')?.value || '20'),
      };
    };
    const saveChatSettings = () => invoke('set_proactive_settings', { settings: getChatProactiveValues() }).catch(() => {});

    // ── Theme toggle ──
    document.getElementById('cs-theme-toggle')?.addEventListener('change', (e) => {
      setTheme(e.target.checked ? 'dark' : 'light');
    });

    document.getElementById('chat-use-openclaw')?.addEventListener('change', (e) => {
      invoke('set_app_setting', { key: 'use_openclaw', value: e.target.checked ? 'true' : 'false' });
    });
    document.getElementById('chat-proactive-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-enabled')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-voice-name')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-quiet-start')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-quiet-end')?.addEventListener('change', saveChatSettings);
    document.getElementById('chat-daily-limit')?.addEventListener('change', saveChatSettings);

    // Interval slider <-> number sync
    const slider = document.getElementById('chat-proactive-slider');
    const numInput = document.getElementById('chat-proactive-number');
    slider?.addEventListener('input', () => {
      numInput.value = slider.value;
    });
    slider?.addEventListener('change', saveChatSettings);
    numInput?.addEventListener('input', () => {
      const v = Math.max(1, Math.min(120, parseInt(numInput.value) || 1));
      if (v <= 60) slider.value = v;
      saveChatSettings();
    });

    // Style toggles
    document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(card => {
      const checkbox = card.querySelector('input[data-style]');
      card.addEventListener('click', (e) => {
        if (e.target.closest('.mini-toggle')) return;
        checkbox.checked = !checkbox.checked;
        card.classList.toggle('enabled', checkbox.checked);
        saveChatSettings();
      });
      checkbox?.addEventListener('change', () => {
        card.classList.toggle('enabled', checkbox.checked);
        saveChatSettings();
      });
    });

    // Select all / none
    document.getElementById('proactive-select-all')?.addEventListener('click', () => {
      document.querySelectorAll('#proactive-styles-grid input[data-style]').forEach(c => { c.checked = true; });
      document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(c => c.classList.add('enabled'));
      saveChatSettings();
    });
    document.getElementById('proactive-select-none')?.addEventListener('click', () => {
      document.querySelectorAll('#proactive-styles-grid input[data-style]').forEach(c => { c.checked = false; });
      document.querySelectorAll('#proactive-styles-grid .proactive-style-card').forEach(c => c.classList.remove('enabled'));
      saveChatSettings();
    });

    document.getElementById('chat-test-voice')?.addEventListener('click', async () => {
      const voice = document.getElementById('chat-voice-name')?.value || 'xenia';
      const btn = document.getElementById('chat-test-voice');
      btn.textContent = 'Говорю...';
      btn.disabled = true;
      try {
        await invoke('speak_text', { text: 'Привет! Я Ханни, твой персональный ассистент.', voice });
      } catch (e) { console.error(e); }
      setTimeout(() => { btn.textContent = 'Прослушать'; btn.disabled = false; }, 3000);
    });

    // TTS server
    document.getElementById('chat-tts-server-save')?.addEventListener('click', async () => {
      const url = document.getElementById('chat-tts-server-url')?.value.trim() || '';
      await invoke('set_app_setting', { key: 'tts_server_url', value: url });
      const statusEl = document.getElementById('chat-tts-server-status');
      if (!url) { if (statusEl) statusEl.textContent = 'Отключён (edge-tts)'; return; }
      try {
        const resp = await fetch(url.replace(/\/$/, '') + '/health');
        const data = await resp.json();
        if (statusEl) statusEl.textContent = `${data.model} | ${data.gpu || 'CPU'}`;
      } catch (e) {
        if (statusEl) statusEl.textContent = 'Недоступен';
      }
    });

    // Auto-check TTS server
    if (ttsServerUrl) {
      try {
        const resp = await fetch(ttsServerUrl.replace(/\/$/, '') + '/health');
        const data = await resp.json();
        const s = document.getElementById('chat-tts-server-status');
        if (s) s.textContent = `${data.model} | ${data.gpu || 'CPU'}`;
      } catch { const s = document.getElementById('chat-tts-server-status'); if (s) s.textContent = 'Недоступен'; }
    }

    // ── Voice Clone handlers ──
    document.getElementById('chat-voice-clone-enabled')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'voice_clone_enabled', value: String(e.target.checked) });
    });
    document.getElementById('chat-voice-clone-sample')?.addEventListener('change', async (e) => {
      await invoke('set_app_setting', { key: 'voice_clone_sample', value: e.target.value });
    });
    document.getElementById('chat-record-voice-sample')?.addEventListener('click', async () => {
      const btn = document.getElementById('chat-record-voice-sample');
      if (!btn) return;
      const name = prompt('Имя для образца голоса:', 'my_voice');
      if (!name) return;
      btn.textContent = 'Записываю (5 сек)...';
      btn.disabled = true;
      try {
        // Record directly via cpal for 5 seconds → save as WAV
        const path = await invoke('record_voice_sample', { name, durationSecs: 5 });
        btn.textContent = 'Сохранено!';
        // Refresh sample list
        const samples = await invoke('list_voice_samples').catch(() => []);
        const sel = document.getElementById('chat-voice-clone-sample');
        if (sel) {
          sel.innerHTML = '<option value="">— нет —</option>' +
            samples.map(s => `<option value="${s.name}">${s.name}</option>`).join('');
          sel.value = name;
        }
        await invoke('set_app_setting', { key: 'voice_clone_sample', value: name });
      } catch (err) {
        btn.textContent = 'Ошибка: ' + String(err).substring(0, 30);
      }
      setTimeout(() => { btn.textContent = 'Записать образец'; btn.disabled = false; }, 3000);
    });

    // ── Training panel handlers ──
    try {
      const pairsPath = '~/Library/Application Support/Hanni/training_pairs.jsonl';
      const content = await invoke('read_file', { path: pairsPath }).catch(() => '');
      const count = content ? content.trim().split('\\n').filter(l => l.trim()).length : 0;
      const pairsEl = document.getElementById('train-pairs-count');
      if (pairsEl) pairsEl.textContent = String(count);
    } catch (_) {
      const pairsEl = document.getElementById('train-pairs-count');
      if (pairsEl) pairsEl.textContent = '0';
    }
    document.getElementById('train-export-btn')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Экспорт...'; btn.disabled = true;
      try { const r = await invoke('export_training_data'); btn.textContent = r.train_count + ' train + ' + r.valid_count + ' valid'; }
      catch (err) { btn.textContent = String(err).substring(0, 30); }
      setTimeout(() => { btn.textContent = 'Экспорт данных'; btn.disabled = false; }, 4000);
    });
    document.getElementById('flywheel-run-btn')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.disabled = true;
      const progress = document.getElementById('flywheel-progress');
      const fill = document.getElementById('flywheel-fill');
      const status = document.getElementById('flywheel-status');
      progress?.classList.remove('hidden');
      btn.textContent = 'Экспорт...';
      if (status) status.textContent = 'Шаг 1/3: Экспорт данных...';
      if (fill) fill.style.width = '15%';
      try {
        await invoke('export_training_data');
        btn.textContent = 'Обучение...';
        if (status) status.textContent = 'Шаг 2/3: Fine-tuning (может занять 10-30 минут)...';
        if (fill) fill.style.width = '40%';
        const r = await invoke('run_flywheel_cycle');
        if (fill) fill.style.width = '100%';
        if (status) status.textContent = `Цикл #${r.cycle_id} завершён: ${r.status} (${r.train_pairs} пар)`;
        btn.textContent = 'Готово!';
      } catch (err) {
        if (fill) fill.style.width = '100%';
        if (status) status.textContent = 'Ошибка: ' + String(err).substring(0, 80);
        btn.textContent = 'Ошибка';
      }
      setTimeout(() => { btn.textContent = 'Запустить цикл'; btn.disabled = false; }, 5000);
    });

    // ── About panel ──
    invoke('get_model_info').catch(() => ({})).then(info => {
      const modelEl = document.getElementById('cs-about-model');
      if (modelEl) modelEl.textContent = info.model_name || '?';
      const mlxEl = document.getElementById('cs-about-mlx');
      if (mlxEl) { mlxEl.textContent = info.server_online ? 'Онлайн' : 'Офлайн'; mlxEl.className = 'about-info-value ' + (info.server_online ? 'online' : 'offline'); }
    });
    fetch('http://127.0.0.1:8235/api/status').then(resp => {
      const apiEl = document.getElementById('cs-about-api');
      if (apiEl) { apiEl.textContent = resp.ok ? 'Активен' : 'Недоступен'; apiEl.className = 'about-info-value ' + (resp.ok ? 'online' : 'offline'); }
    }).catch(() => {
      const apiEl = document.getElementById('cs-about-api');
      if (apiEl) { apiEl.textContent = 'Недоступен'; apiEl.className = 'about-info-value offline'; }
    });
    document.getElementById('cs-about-check-update')?.addEventListener('click', async (e) => {
      const btn = e.target; btn.textContent = 'Проверяю...'; btn.disabled = true;
      try { const r = await invoke('check_update'); btn.textContent = r; }
      catch (err) { btn.textContent = 'Ошибка'; }
      setTimeout(() => { btn.textContent = 'Проверить обновления'; btn.disabled = false; }, 4000);
    });

  } catch (e) {
    el.innerHTML = `<div style="color:var(--color-red);font-size:14px;">Ошибка: ${e}</div>`;
  }
}

// Helper: render memory items (just the list — no listener re-registration)
function _csMemRenderList(memories) {
  const list = document.getElementById('cs-mem-list');
  if (!list) return;
  const countEl = document.getElementById('cs-mem-count');
  if (countEl) countEl.textContent = `${memories.length} фактов`;
  list.innerHTML = memories.map(m => `<div class="memory-item" data-mem-id="${m.id}">
    <span class="memory-item-category memory-cat-${m.category || 'other'}">${escapeHtml(m.category)}</span>
    <span class="memory-item-key">${escapeHtml(m.key)}</span>
    <span class="memory-item-value">${escapeHtml(m.value)}</span>
    <div class="memory-item-actions">
      <button class="memory-item-btn memory-edit-btn" data-csedit="${m.id}" title="Редактировать">&#9998;</button>
      <button class="memory-item-btn" data-csdel="${m.id}" title="Удалить">&times;</button>
    </div>
  </div>`).join('') || '<div style="color:var(--text-faint);font-size:12px;padding:8px;">Ничего не найдено</div>';
}

// Setup memory panel: event delegation + search + add (called ONCE)
function _chatSettingsSetupMemory(memories) {
  // Reload helper
  const reloadMem = async () => {
    const q = document.getElementById('cs-mem-search')?.value || null;
    const updated = await invoke('get_all_memories', { search: q }).catch(() => []);
    _csMemRenderList(updated);
  };

  // Event delegation on the list container (handles delete + edit for all items)
  const list = document.getElementById('cs-mem-list');
  if (list) {
    list.addEventListener('click', async (e) => {
      const delBtn = e.target.closest('[data-csdel]');
      if (delBtn) {
        if (await confirmModal()) {
          await invoke('delete_memory', { id: parseInt(delBtn.dataset.csdel) }).catch(e => console.error('delete_memory error:', e));
          reloadMem();
        }
        return;
      }
      const editBtn = e.target.closest('[data-csedit]');
      if (editBtn) {
        const id = parseInt(editBtn.dataset.csedit);
        // Fetch current value from DB to avoid stale data
        const allMem = await invoke('get_all_memories', { search: null }).catch(() => []);
        const m = allMem.find(x => x.id === id);
        if (!m) return;
        const overlay = document.createElement('div');
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal modal-compact">
          <div class="modal-title">Редактировать факт</div>
          <div class="form-group"><label class="form-label">Категория</label>
            <select class="form-select memory-edit-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}" ${c === m.category ? 'selected' : ''}>${c}</option>`).join('')}</select>
          </div>
          <div class="form-group"><label class="form-label">Ключ</label>
            <input class="form-input memory-edit-key" value="${escapeHtml(m.key)}" placeholder="Ключ">
          </div>
          <div class="form-group"><label class="form-label">Значение</label>
            <textarea class="form-input memory-edit-val" placeholder="Значение" rows="3" style="resize:vertical;">${escapeHtml(m.value)}</textarea>
          </div>
          <div class="modal-actions">
            <button class="btn-secondary mem-cancel">Отмена</button>
            <button class="btn-primary mem-save">Сохранить</button>
          </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
        overlay.addEventListener('click', ev => { if (ev.target === overlay) overlay.remove(); });
        overlay.querySelector('.mem-save').onclick = async () => {
          const cat = overlay.querySelector('.memory-edit-cat').value;
          const key = overlay.querySelector('.memory-edit-key').value.trim();
          const val = overlay.querySelector('.memory-edit-val').value.trim();
          if (!key || !val) return;
          try { await invoke('update_memory', { id, category: cat, key, value: val }); } catch (err) { console.error(err); }
          overlay.remove();
          reloadMem();
        };
      }
    });
  }

  // Add button (once)
  document.getElementById('cs-mem-add-btn')?.addEventListener('click', () => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `<div class="modal modal-compact">
      <div class="modal-title">Новый факт</div>
      <div class="form-group"><label class="form-label">Категория</label>
        <select class="form-select memory-add-cat">${MEMORY_CATEGORIES.map(c => `<option value="${c}">${c}</option>`).join('')}</select>
      </div>
      <div class="form-group"><label class="form-label">Ключ</label>
        <input class="form-input memory-add-key" placeholder="напр. имя, привычка" autocomplete="off">
      </div>
      <div class="form-group"><label class="form-label">Значение</label>
        <input class="form-input memory-add-val" placeholder="Значение факта" autocomplete="off">
      </div>
      <div class="modal-actions">
        <button class="btn-secondary mem-cancel">Отмена</button>
        <button class="btn-primary mem-save">Добавить</button>
      </div>
    </div>`;
    document.body.appendChild(overlay);
    overlay.querySelector('.mem-cancel').onclick = () => overlay.remove();
    overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
    overlay.querySelector('.mem-save').onclick = async () => {
      const cat = overlay.querySelector('.memory-add-cat').value;
      const key = overlay.querySelector('.memory-add-key').value.trim();
      const val = overlay.querySelector('.memory-add-val').value.trim();
      if (!key || key.length < 2 || !val || val.length < 2) { overlay.querySelector('.memory-add-key').style.borderColor = !key || key.length < 2 ? 'var(--accent)' : ''; return; }
      try { await invoke('memory_remember', { category: cat, key, value: val }); } catch (err) { console.error(err); }
      overlay.remove();
      reloadMem();
    };
  });

  // Search (once, with debounce)
  let searchTimeout;
  document.getElementById('cs-mem-search')?.addEventListener('input', (e) => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(async () => {
      const q = e.target.value;
      const results = await invoke('get_all_memories', { search: q || null }).catch(() => []);
      _csMemRenderList(results);
    }, 300);
  });
}

// ── Chat helpers ──

function scrollDown() {
  if (S._scrollRAF) return;
  S._scrollRAF = requestAnimationFrame(() => {
    chat.scrollTop = chat.scrollHeight;
    S._scrollRAF = null;
  });
}

// Scroll-to-bottom floating button
const scrollBottomBtn = document.getElementById('scroll-bottom-btn');
chat.addEventListener('scroll', () => {
  const distFromBottom = chat.scrollHeight - chat.scrollTop - chat.clientHeight;
  scrollBottomBtn?.classList.toggle('visible', distFromBottom > 200);
});
scrollBottomBtn?.addEventListener('click', () => {
  chat.scrollTo({ top: chat.scrollHeight, behavior: 'smooth' });
});

function addMsg(role, text, isVoice = false) {
  if (role === 'bot') {
    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const div = document.createElement('div');
    div.className = 'msg bot markdown-body';
    div.innerHTML = renderMarkdown(text);
    wrapper.appendChild(div);
    const ttsBtn = document.createElement('button');
    ttsBtn.className = 'tts-btn';
    ttsBtn.innerHTML = '&#9654;';
    ttsBtn.title = 'Озвучить';
    ttsBtn.addEventListener('click', () => toggleTTS(ttsBtn, text));
    wrapper.appendChild(ttsBtn);
    chat.appendChild(wrapper);
    scrollDown();
    return div;
  }
  const wrapper = document.createElement('div');
  wrapper.className = 'msg-wrapper user-wrapper';
  const div = document.createElement('div');
  div.className = `msg ${role}`;
  if (isVoice && role === 'user') {
    const mic = document.createElement('span');
    mic.className = 'voice-indicator';
    mic.textContent = '\u{1F3A4} ';
    div.appendChild(mic);
    div.appendChild(document.createTextNode(text));
  } else {
    div.textContent = text;
  }
  wrapper.appendChild(div);

  // CH4: Edit button for user messages
  if (role === 'user') {
    const editBtn = document.createElement('button');
    editBtn.className = 'feedback-btn edit-msg-btn';
    editBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 000-1.41l-2.34-2.34a1 1 0 00-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"/></svg>';
    editBtn.title = 'Редактировать';
    editBtn.addEventListener('click', () => {
      const input = document.getElementById('input');
      input.value = text;
      input.focus();
      // Find this wrapper's history index and remove everything after it
      const allWrappers = [...chat.querySelectorAll('.msg-wrapper, .msg.user, .action-result, .memory-toast')];
      const idx = allWrappers.indexOf(wrapper);
      if (idx >= 0) {
        for (let i = allWrappers.length - 1; i > idx; i--) allWrappers[i].remove();
      }
      wrapper.remove();
      // Find and truncate history to this user message
      const wrapperIdx = parseInt(wrapper.dataset.historyIdx || '-1', 10);
      if (wrapperIdx >= 0) {
        S.history.length = wrapperIdx;
      } else {
        // Fallback: remove from the last user message matching this text
        for (let i = S.history.length - 1; i >= 0; i--) {
          if (S.history[i].role === 'user' && S.history[i].content === text) {
            S.history.length = i;
            break;
          }
        }
      }
    });
    wrapper.appendChild(editBtn);
  }

  chat.appendChild(wrapper);
  scrollDown();
  return div;
}

function addFeedbackButtons(wrapper, conversationId, messageIndex, botText) {
  const actions = document.createElement('div');
  actions.className = 'msg-actions';

  // Copy button
  const copyBtn = document.createElement('button');
  copyBtn.className = 'feedback-btn copy-btn';
  copyBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></svg>';
  copyBtn.title = 'Копировать';
  copyBtn.addEventListener('click', () => {
    navigator.clipboard.writeText(botText || '');
    copyBtn.classList.add('copied');
    copyBtn.title = 'Скопировано!';
    setTimeout(() => { copyBtn.classList.remove('copied'); copyBtn.title = 'Копировать'; }, 1500);
  });

  // Regenerate button
  const regenBtn = document.createElement('button');
  regenBtn.className = 'feedback-btn regen-btn';
  regenBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M17.65 6.35A7.96 7.96 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/></svg>';
  regenBtn.title = 'Перегенерировать';
  regenBtn.addEventListener('click', async () => {
    if (S.busy) return;
    // Remove last bot message from history and re-send
    while (S.history.length > 0 && S.history[S.history.length - 1].role === 'assistant') {
      S.history.pop();
    }
    // Remove wrapper from DOM
    wrapper.remove();
    // Re-send
    send();
  });

  const thumbUp = document.createElement('button');
  thumbUp.className = 'feedback-btn thumb-up';
  thumbUp.innerHTML = '<svg viewBox="0 0 24 24"><path d="M2 20h2V10H2v10zm20-9a2 2 0 0 0-2-2h-6.31l.95-4.57.03-.32a1.5 1.5 0 0 0-.44-1.06L13.17 2 7.59 7.59A2 2 0 0 0 7 9v10a2 2 0 0 0 2 2h9c.83 0 1.54-.5 1.84-1.22l3.02-7.05c.09-.23.14-.47.14-.73v-2z"/></svg>';
  thumbUp.title = 'Хороший ответ';

  const thumbDown = document.createElement('button');
  thumbDown.className = 'feedback-btn thumb-down';
  thumbDown.innerHTML = '<svg viewBox="0 0 24 24"><path d="M22 4h-2v10h2V4zM2 13a2 2 0 0 0 2 2h6.31l-.95 4.57-.03.32c0 .4.17.77.44 1.06L10.83 22l5.58-5.59A2 2 0 0 0 17 15V5a2 2 0 0 0-2-2H6c-.83 0-1.54.5-1.84 1.22L1.14 11.27c-.09.23-.14.47-.14.73v2z"/></svg>';
  thumbDown.title = 'Плохой ответ';

  const handleClick = async (btn, rating) => {
    const isActive = btn.classList.contains('active');
    thumbUp.classList.remove('active');
    thumbDown.classList.remove('active');
    if (!isActive) {
      btn.classList.add('active');
      try {
        await invoke('rate_message', { conversationId, messageIndex, rating });
      } catch (e) {
        console.error('Rate error:', e);
      }
    } else {
      try {
        await invoke('rate_message', { conversationId, messageIndex, rating: 0 });
      } catch (e) {
        console.error('Rate error:', e);
      }
    }
  };

  thumbUp.addEventListener('click', () => handleClick(thumbUp, 1));
  thumbDown.addEventListener('click', () => handleClick(thumbDown, -1));

  actions.appendChild(copyBtn);
  actions.appendChild(regenBtn);
  actions.appendChild(thumbUp);
  actions.appendChild(thumbDown);
  wrapper.appendChild(actions);
  return { thumbUp, thumbDown };
}

function addProactiveFeedbackButtons(wrapper, proactiveId, botText, useMessageTable = false) {
  const actions = document.createElement('div');
  actions.className = 'msg-actions';

  const copyBtn = document.createElement('button');
  copyBtn.className = 'feedback-btn copy-btn';
  copyBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/></svg>';
  copyBtn.title = 'Копировать';
  copyBtn.addEventListener('click', () => {
    navigator.clipboard.writeText(botText || '');
    copyBtn.classList.add('copied');
    copyBtn.title = 'Скопировано!';
    setTimeout(() => { copyBtn.classList.remove('copied'); copyBtn.title = 'Копировать'; }, 1500);
  });

  const thumbUp = document.createElement('button');
  thumbUp.className = 'feedback-btn thumb-up';
  thumbUp.innerHTML = '<svg viewBox="0 0 24 24"><path d="M2 20h2V10H2v10zm20-9a2 2 0 0 0-2-2h-6.31l.95-4.57.03-.32a1.5 1.5 0 0 0-.44-1.06L13.17 2 7.59 7.59A2 2 0 0 0 7 9v10a2 2 0 0 0 2 2h9c.83 0 1.54-.5 1.84-1.22l3.02-7.05c.09-.23.14-.47.14-.73v-2z"/></svg>';
  thumbUp.title = 'Хороший ответ';

  const thumbDown = document.createElement('button');
  thumbDown.className = 'feedback-btn thumb-down';
  thumbDown.innerHTML = '<svg viewBox="0 0 24 24"><path d="M22 4h-2v10h2V4zM2 13a2 2 0 0 0 2 2h6.31l-.95 4.57-.03.32c0 .4.17.77.44 1.06L10.83 22l5.58-5.59A2 2 0 0 0 17 15V5a2 2 0 0 0-2-2H6c-.83 0-1.54.5-1.84 1.22L1.14 11.27c-.09.23-.14.47-.14.73v2z"/></svg>';
  thumbDown.title = 'Плохой ответ';

  const handleClick = async (btn, rating) => {
    const isActive = btn.classList.contains('active');
    thumbUp.classList.remove('active');
    thumbDown.classList.remove('active');
    const r = isActive ? 0 : rating;
    if (!isActive) btn.classList.add('active');
    try {
      if (useMessageTable) {
        await invoke('rate_proactive_message', { id: proactiveId, rating: r });
      } else {
        await invoke('rate_proactive', { proactiveId, rating: r });
      }
    } catch (e) {
      console.error('Rate proactive error:', e);
    }
  };

  thumbUp.addEventListener('click', () => handleClick(thumbUp, 1));
  thumbDown.addEventListener('click', () => handleClick(thumbDown, -1));

  actions.appendChild(copyBtn);
  actions.appendChild(thumbUp);
  actions.appendChild(thumbDown);
  wrapper.appendChild(actions);
  return { thumbUp, thumbDown };
}

// ── File attachment ──

attachBtn?.addEventListener('click', () => fileInput.click());

fileInput.addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;

  if (file.size > 512000) {
    addMsg('bot', 'Файл слишком большой (макс 500KB)');
    fileInput.value = '';
    return;
  }

  const text = await file.text();
  S.attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `\u{1F4CE} ${file.name}`;
  attachPreview.style.display = 'block';
  fileInput.value = '';
});

attachPreview.addEventListener('click', () => {
  S.attachedFile = null;
  attachPreview.style.display = 'none';
});

// ── Drag & drop ──

document.body.addEventListener('dragover', (e) => {
  e.preventDefault();
  document.body.classList.add('dragover');
});

document.body.addEventListener('dragleave', () => {
  document.body.classList.remove('dragover');
});

document.body.addEventListener('drop', async (e) => {
  e.preventDefault();
  document.body.classList.remove('dragover');
  const file = e.dataTransfer.files[0];
  if (!file) return;

  if (file.size > 512000) {
    addMsg('bot', 'Файл слишком большой (макс 500KB)');
    return;
  }

  const text = await file.text();
  S.attachedFile = { name: file.name, content: text };
  attachPreview.textContent = `\u{1F4CE} ${file.name}`;
  attachPreview.style.display = 'block';
});

// ── Agent step indicator ──

function showAgentIndicator(step) {
  const div = document.createElement('div');
  div.className = 'agent-step';
  div.textContent = `шаг ${step}...`;
  chat.appendChild(div);
  scrollDown();
  return div;
}

// ── TTS ──

async function toggleTTS(btn, text) {
  if (S.isSpeaking) {
    await stopAllTTS();
    return;
  }
  S.isSpeaking = true;
  btn.classList.add('speaking');
  btn.innerHTML = '&#9632;';
  document.getElementById('stop-tts')?.classList.remove('hidden');
  try {
    let voice = 'xenia';
    try {
      const ps = await invoke('get_proactive_settings');
      voice = ps.voice_name || 'xenia';
    } catch (_) {}
    // V8: Check if voice clone is enabled
    const cloneEnabled = await invoke('get_app_setting', { key: 'voice_clone_enabled' }).catch(() => null);
    const cloneSample = await invoke('get_app_setting', { key: 'voice_clone_sample' }).catch(() => null);
    if (cloneEnabled === 'true' && cloneSample) {
      await invoke('speak_clone_blocking', { text, sampleName: cloneSample });
    } else {
      await invoke('speak_text_blocking', { text, voice });
    }
    btn.classList.remove('speaking');
    btn.innerHTML = '&#9654;';
    S.isSpeaking = false;
    document.getElementById('stop-tts')?.classList.add('hidden');
  } catch (_) {
    btn.classList.remove('speaking');
    btn.innerHTML = '&#9654;';
    S.isSpeaking = false;
    document.getElementById('stop-tts')?.classList.add('hidden');
  }
}

async function stopAllTTS() {
  await invoke('stop_speaking').catch(() => {});
  document.querySelectorAll('.tts-btn.speaking').forEach(b => {
    b.classList.remove('speaking');
    b.innerHTML = '&#9654;';
  });
  S.isSpeaking = false;
  document.getElementById('stop-tts')?.classList.add('hidden');
}

// Stop TTS button
document.getElementById('stop-tts')?.addEventListener('click', stopAllTTS);

// ── streamChat ──

async function streamChat(botDiv, t0, callMode = false) {
  const cursor = document.createElement('span');
  cursor.className = 'cursor';
  botDiv.appendChild(cursor);

  let firstToken = 0;
  let tokens = 0;
  let fullReply = '';
  let reasoningText = '';
  let toolCalls = [];
  let finishReason = null;

  // Reasoning collapsible block (thinking mode)
  let reasoningDetails = null;
  let reasoningContent = null;

  let scrollRAF = null;
  const scrollDownThrottled = () => {
    if (!scrollRAF) {
      scrollRAF = requestAnimationFrame(() => { scrollDown(); scrollRAF = null; });
    }
  };

  const unlistenReasoning = await listen('chat-reasoning', (event) => {
    if (!firstToken) firstToken = performance.now() - t0;
    const token = event.payload.token;
    reasoningText += token;
    // Create collapsible block on first reasoning token
    if (!reasoningDetails) {
      reasoningDetails = document.createElement('details');
      reasoningDetails.className = 'thinking-block';
      reasoningDetails.open = true;
      const summary = document.createElement('summary');
      summary.textContent = '\u{1F914} Думает...';
      reasoningDetails.appendChild(summary);
      reasoningContent = document.createElement('div');
      reasoningContent.className = 'thinking-content';
      reasoningDetails.appendChild(reasoningContent);
      botDiv.insertBefore(reasoningDetails, cursor);
    }
    reasoningContent.appendChild(document.createTextNode(token));
    scrollDownThrottled();
  });

  const unlistenReasoningDone = await listen('chat-reasoning-done', () => {
    if (reasoningDetails) {
      reasoningDetails.open = false; // collapse when done
      reasoningDetails.querySelector('summary').textContent = '\u{1F914} Рассуждения';
    }
  });

  const unlisten = await listen('chat-token', (event) => {
    if (!firstToken) firstToken = performance.now() - t0;
    tokens++;
    const token = event.payload.token;
    fullReply += token;
    botDiv.insertBefore(document.createTextNode(token), cursor);
    scrollDownThrottled();
  });

  const unlistenDone = await listen('chat-done', () => {
    // Fallback: close reasoning block if model ran out of tokens before content
    if (reasoningDetails && reasoningDetails.open) {
      reasoningDetails.open = false;
      reasoningDetails.querySelector('summary').textContent = '\u{1F914} Рассуждения';
    }
  });

  try {
    const msgs = S.history.slice(-20).map(normalizeHistoryMessage);
    const resultJson = await invoke('chat', { messages: msgs, callMode, conversationId: S.currentConversationId });
    // Parse ChatResult JSON from Rust
    try {
      const chatResult = JSON.parse(resultJson);
      if (chatResult.tool_calls && chatResult.tool_calls.length > 0) {
        toolCalls = chatResult.tool_calls;
      }
      finishReason = chatResult.finish_reason || null;
      // fullReply was already built by streaming tokens; use chatResult.text as fallback
      if (!fullReply && chatResult.text) {
        fullReply = chatResult.text;
      }
    } catch (_) {
      // If not valid JSON, treat as plain text (backward compat)
      if (!fullReply && typeof resultJson === 'string') {
        fullReply = resultJson;
      }
    }
  } catch (e) {
    if (!fullReply) {
      const errMsg = String(e);
      if (errMsg.includes('connection') || errMsg.includes('Connection') || errMsg.includes('refused')) {
        botDiv.textContent = 'MLX сервер недоступен — модель загружается...';
      } else {
        botDiv.textContent = 'Ошибка: ' + errMsg;
      }
    }
  }

  unlisten();
  unlistenDone();
  unlistenReasoning();
  unlistenReasoningDone();
  cursor.remove();

  // Re-render as Markdown after streaming completes
  if (fullReply) {
    botDiv.classList.add('markdown-body');
    botDiv.innerHTML = renderMarkdown(fullReply);
  }

  return { fullReply, tokens, firstToken, toolCalls, finishReason };
}

// ── Send message ──

async function send() {
  const text = input.value.trim();
  if (!text || S.busy) return;

  S.busy = true;
  sendBtn.disabled = true;
  input.value = '';
  input.style.height = 'auto';

  try {

  // Report user chat activity for adaptive timing
  invoke('report_user_chat_activity').catch(() => {});
  // If user replies within 10 min of a proactive message, report engagement
  if (S.lastProactiveTime && (Date.now() - S.lastProactiveTime) < 600000) {
    invoke('report_proactive_engagement').catch(() => {});
    S.lastProactiveTime = 0;
  }

  // Build message with optional file
  const isVoice = S.lastMessageWasVoice;
  const sttTime = S.lastSttTimeMs;
  S.lastMessageWasVoice = false;
  S.lastSttTimeMs = 0;

  removeChatWelcomeCard();

  let userContent = text;
  if (S.attachedFile) {
    userContent += `\n\n\u{1F4CE} Файл: ${S.attachedFile.name}\n\`\`\`\n${S.attachedFile.content}\n\`\`\``;
    addMsg('user', `${text}\n\u{1F4CE} ${S.attachedFile.name}`);
    S.attachedFile = null;
    attachPreview.style.display = 'none';
  } else {
    addMsg('user', text, isVoice);
  }

  // If previous message was a proactive/autonomous message, add context hint
  // so the model focuses on the user's reply, not on echoing itself
  const prevMsg = S.history[S.history.length - 1];
  if (prevMsg && prevMsg.proactive) {
    // Rewrite the proactive message to include a marker for the model
    prevMsg.content = `[Автономное сообщение Ханни]: ${prevMsg.content}`;
  }

  S.history.push({ role: 'user', content: userContent });
  // Set history index on user wrapper for edit support
  { const lastUserWrapper = chat.querySelector('.user-wrapper:last-of-type');
    if (lastUserWrapper) lastUserWrapper.dataset.historyIdx = String(S.history.length - 1); }

  const MAX_ITERATIONS = 5;
  const t0 = performance.now();
  let totalTokens = 0;
  let firstToken = 0;
  let iteration = 0;

  while (iteration < MAX_ITERATIONS) {
    iteration++;

    // Show step indicator for iterations after the first
    if (iteration > 1) {
      showAgentIndicator(iteration);
    }

    // Create bot message div with TTS wrapper
    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const botDiv = document.createElement('div');
    botDiv.className = 'msg bot';
    wrapper.appendChild(botDiv);
    chat.appendChild(wrapper);
    scrollDown();

    // Stream model response
    const result = await streamChat(botDiv, t0);
    if (!firstToken && result.firstToken) firstToken = result.firstToken;
    totalTokens += result.tokens;

    // Primary path: native tool calls from the model
    if (result.toolCalls && result.toolCalls.length > 0) {
      // Push assistant message with tool_calls into history
      const assistantMsg = { role: 'assistant', content: result.fullReply || null, tool_calls: result.toolCalls };
      S.history.push(assistantMsg);
      wrapper.dataset.historyIdx = String(S.history.length - 1);

      // Mark as intermediate (tool calls, not final answer)
      botDiv.classList.add('intermediate');

      // Execute each tool call and push results
      for (const tc of result.toolCalls) {
        let args;
        try { args = JSON.parse(tc.function.arguments); } catch (_) { args = {}; }
        // CH7: Show streaming action indicator
        const indicatorDiv = document.createElement('div');
        indicatorDiv.className = 'action-indicator';
        indicatorDiv.textContent = `Выполняю: ${tc.function.name}...`;
        chat.appendChild(indicatorDiv);
        scrollDown();
        // Inject action type for executeAction compatibility
        args.action = tc.function.name;
        const actionJson = JSON.stringify(args);
        const { success, result: actionResult } = await tabLoaders.executeAction(actionJson);
        indicatorDiv.remove();
        const actionDiv = document.createElement('div');
        actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
        actionDiv.textContent = actionResult;
        chat.appendChild(actionDiv);
        scrollDown();

        // Push tool result into history
        S.history.push({
          role: 'tool',
          tool_call_id: tc.id,
          name: tc.function.name,
          content: String(actionResult)
        });
      }
      // Continue loop — model will respond with a summary
      continue;
    }

    if (!result.fullReply) break;

    S.history.push({ role: 'assistant', content: result.fullReply });
    wrapper.dataset.historyIdx = String(S.history.length - 1);

    // Fallback path: parse ```action blocks from text (backward compat)
    const actions = tabLoaders.parseAndExecuteActions(result.fullReply);

    // No actions — this is the final answer, stop the loop
    if (actions.length === 0) break;

    // Mark this response as intermediate (it contained actions, not a final answer)
    botDiv.classList.add('intermediate');

    // Execute actions and show results
    const results = [];
    for (const actionJson of actions) {
      // CH7: Show action indicator
      let actionName = 'action';
      try { const a = JSON.parse(actionJson); actionName = a.action || a.type || 'action'; } catch (_) {}
      const indicatorDiv = document.createElement('div');
      indicatorDiv.className = 'action-indicator';
      indicatorDiv.textContent = `Выполняю: ${actionName}...`;
      chat.appendChild(indicatorDiv);
      scrollDown();
      const { success, result: actionResult } = await tabLoaders.executeAction(actionJson);
      indicatorDiv.remove();
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      scrollDown();
      results.push(actionResult);
    }

    // Feed results back into history so the model sees them
    S.history.push({ role: 'user', content: `[Action result: ${results.join('; ')}]` });
  }

  // Add TTS button to last bot wrapper
  const lastWrapper = chat.querySelector('.msg-wrapper:last-of-type');
  if (lastWrapper && !lastWrapper.querySelector('.tts-btn')) {
    const lastBotDiv = lastWrapper.querySelector('.msg.bot');
    if (lastBotDiv) {
      const ttsBtn = document.createElement('button');
      ttsBtn.className = 'tts-btn';
      ttsBtn.innerHTML = '&#9654;';
      ttsBtn.title = 'Озвучить';
      const ttsText = lastBotDiv.textContent;
      ttsBtn.addEventListener('click', () => toggleTTS(ttsBtn, ttsText));
      lastWrapper.appendChild(ttsBtn);
    }
  }

  // Show timing
  const total = ((performance.now() - t0) / 1000).toFixed(1);
  const ttft = firstToken ? (firstToken / 1000).toFixed(1) : '?';
  const timing = document.createElement('div');
  timing.className = 'timing';
  const stepInfo = iteration > 1 ? ` \u00B7 ${iteration} steps` : '';
  const sttInfo = isVoice && sttTime ? `STT ${(sttTime / 1000).toFixed(1)}s \u00B7 ` : '';
  timing.textContent = `${sttInfo}${ttft}s first token \u00B7 ${total}s total \u00B7 ${totalTokens} tokens${stepInfo}`;
  chat.appendChild(timing);
  scrollDown();

  // Post-chat: incremental save + extract facts in background
  (async () => {
    try {
      if (S.currentConversationId) {
        await invoke('update_conversation', { id: S.currentConversationId, messages: S.history });
      } else {
        S.currentConversationId = await invoke('save_conversation', { messages: S.history });
      }
      // Add feedback buttons to bot messages that have a history index
      if (S.currentConversationId) {
        chat.querySelectorAll('.msg-wrapper[data-history-idx]').forEach(w => {
          if (w.querySelector('.feedback-btn')) return;
          const idx = parseInt(w.dataset.historyIdx, 10);
          if (!isNaN(idx) && getRole(S.history[idx]) === 'assistant') {
            addFeedbackButtons(w, S.currentConversationId, idx, S.history[idx]?.content || '');
          }
        });
      }
      if (S.history.length >= 2) {
        await invoke('process_conversation_end', { messages: S.history, conversationId: S.currentConversationId });
      }
      loadConversationsList();
    } catch (_) {}
  })();

  } catch (err) {
    console.error('send() error:', err);
    addMsg('bot', 'Ошибка: ' + (err.message || err));
  } finally {
    S.busy = false;
    sendBtn.disabled = false;
    input.focus();
  }
}

// ── New Chat ──

async function newChat() {
  if (S.busy) S.busy = false;  // allow new chat even during active stream
  // Save current conversation before clearing
  await autoSaveConversation();
  S.currentConversationId = null;
  S.history = [];
  chat.innerHTML = '';
  renderChatWelcomeCard();
  loadConversationsList();
  input.focus();
}

document.getElementById('new-chat')?.addEventListener('click', newChat);

// ── Input tools: plus button → dropdown → pills ──
const TOOLS_CONFIG = [
  { id: 'thinking', key: 'enable_thinking', label: 'Думать', icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2a6.5 6.5 0 0 1 4.93 10.73L15 14H9l.57-1.27A6.5 6.5 0 0 1 9.5 2z"/><path d="M9 14v2a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-2"/><line x1="10" y1="18" x2="14" y2="18"/></svg>' },
  { id: 'selfrefine', key: 'enable_self_refine', label: 'Проверить', icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><polyline points="9 12 11 14 15 10"/></svg>' },
  { id: 'websearch', key: 'enable_web_search', label: 'Поиск', icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>' },
];
const toolsState = {};
const plusBtn = document.getElementById('input-plus-btn');
const pillsContainer = document.getElementById('input-pills');

function renderPills() {
  pillsContainer.innerHTML = '';
  for (const t of TOOLS_CONFIG) {
    if (!toolsState[t.id]) continue;
    const pill = document.createElement('span');
    pill.className = 'input-pill';
    pill.dataset.tool = t.id;
    pill.innerHTML = `<svg class="pill-icon" width="12" height="12">${t.icon}</svg>${t.label}<span class="pill-close" data-tool="${t.id}">&times;</span>`;
    pill.querySelector('.pill-close').addEventListener('click', (e) => {
      e.stopPropagation();
      toggleTool(t.id);
    });
    pillsContainer.appendChild(pill);
  }
}

async function toggleTool(id) {
  toolsState[id] = !toolsState[id];
  const cfg = TOOLS_CONFIG.find(t => t.id === id);
  if (cfg) await invoke('set_app_setting', { key: cfg.key, value: toolsState[id] ? 'true' : 'false' }).catch(() => {});
  renderPills();
  const dd = document.getElementById('input-tools-dropdown');
  if (dd) dd.querySelector(`[data-tool="${id}"]`)?.classList.toggle('active', toolsState[id]);
}

function showToolsDropdown() {
  let dd = document.getElementById('input-tools-dropdown');
  if (dd) { closeToolsDropdown(); return; }
  plusBtn.classList.add('open');
  dd = document.createElement('div');
  dd.id = 'input-tools-dropdown';
  const checkSvg = '<svg class="tools-dd-check" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 8 7 12 13 4"/></svg>';
  for (const t of TOOLS_CONFIG) {
    const item = document.createElement('button');
    item.className = 'tools-dd-item' + (toolsState[t.id] ? ' active' : '');
    item.dataset.tool = t.id;
    item.innerHTML = `<svg width="16" height="16">${t.icon}</svg>${t.label}${checkSvg}`;
    item.addEventListener('click', () => toggleTool(t.id));
    dd.appendChild(item);
  }
  const sep = document.createElement('div');
  sep.className = 'tools-dd-sep';
  dd.appendChild(sep);
  const attachItem = document.createElement('button');
  attachItem.className = 'tools-dd-item';
  attachItem.innerHTML = `<svg viewBox="0 0 16 16" fill="none"><path d="M13.5 7.5l-5.793 5.793a3.5 3.5 0 01-4.95-4.95L9.05 2.05a2 2 0 012.828 2.828L5.586 11.17a.5.5 0 01-.707-.707L11.17 4.17" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>Прикрепить файл`;
  attachItem.addEventListener('click', () => { fileInput.click(); closeToolsDropdown(); });
  dd.appendChild(attachItem);
  document.getElementById('input-wrapper').appendChild(dd);
  setTimeout(() => document.addEventListener('mousedown', handleOutsideClick), 10);
}

function closeToolsDropdown() {
  const dd = document.getElementById('input-tools-dropdown');
  if (dd) dd.remove();
  plusBtn.classList.remove('open');
  document.removeEventListener('mousedown', handleOutsideClick);
}

function handleOutsideClick(e) {
  const dd = document.getElementById('input-tools-dropdown');
  if (dd && !dd.contains(e.target) && e.target !== plusBtn && !plusBtn.contains(e.target)) closeToolsDropdown();
}

plusBtn.addEventListener('click', showToolsDropdown);

// Load saved state
(async () => {
  const [thinkVal, refineVal, webVal] = await Promise.all([
    invoke('get_app_setting', { key: 'enable_thinking' }).catch(() => null),
    invoke('get_app_setting', { key: 'enable_self_refine' }).catch(() => null),
    invoke('get_app_setting', { key: 'enable_web_search' }).catch(() => null),
  ]);
  toolsState.thinking = thinkVal === 'true';
  toolsState.selfrefine = refineVal === 'true';
  toolsState.websearch = webVal === 'true';
  renderPills();
})();

sendBtn.addEventListener('click', send);
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    send();
  }
});
// Auto-resize textarea as user types
input.addEventListener('input', () => {
  input.style.height = 'auto';
  input.style.height = Math.min(input.scrollHeight, 150) + 'px';
});

// ── showStub ──

function showStub(containerId, icon, label, desc) {
  const el = document.getElementById(containerId);
  if (el) el.innerHTML = `<div class="tab-stub">
    <div class="tab-stub-icon">${icon}</div>
    <div class="tab-stub-title">${label}</div>
    ${desc ? `<div class="tab-stub-desc">${desc}</div>` : ''}
    <span class="tab-stub-badge">Скоро</span>
  </div>`;
}

// ── Chat Welcome Card ──

function removeChatWelcomeCard() {
  document.getElementById('chat-welcome-card')?.remove();
}

async function renderChatWelcomeCard() {
  if (chat.querySelector('.msg, .msg-wrapper, .user-wrapper')) return;
  removeChatWelcomeCard();

  const now = new Date();
  const greeting = now.getHours() < 12 ? 'Доброе утро' : now.getHours() < 18 ? 'Добрый день' : 'Добрый вечер';
  const dateStr = now.toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

  let statsHtml = `
    <div class="welcome-stats">
      <div class="welcome-stat"><div class="welcome-stat-value">\u2014</div><div class="welcome-stat-label">Активности</div></div>
      <div class="welcome-stat"><div class="welcome-stat-value">\u2014</div><div class="welcome-stat-label">Фокус</div></div>
      <div class="welcome-stat"><div class="welcome-stat-value">\u2014</div><div class="welcome-stat-label">Заметки</div></div>
      <div class="welcome-stat"><div class="welcome-stat-value">\u2014</div><div class="welcome-stat-label">События</div></div>
    </div>`;
  let focusBanner = '';
  let eventsHtml = '';

  const card = document.createElement('div');
  card.id = 'chat-welcome-card';
  card.innerHTML = `
    <div class="welcome-greeting">${greeting}!</div>
    <div class="welcome-date">${dateStr}</div>
    ${focusBanner}${statsHtml}${eventsHtml}
    <div class="welcome-actions">
      <button class="welcome-action-btn" data-nav="notes">Заметка</button>
      <button class="welcome-action-btn" data-nav="focus">Фокус</button>
      <button class="welcome-action-btn" data-nav="calendar">Календарь</button>
      <button class="welcome-action-btn" data-nav="health">Здоровье</button>
    </div>`;
  chat.appendChild(card);

  // Attach navigation handlers (replacing inline onclick)
  card.querySelectorAll('.welcome-action-btn[data-nav]').forEach(btn => {
    btn.addEventListener('click', () => tabLoaders.switchTab(btn.dataset.nav));
  });

  // Load real data async
  try {
    const data = await invoke('get_dashboard_data');
    if (!document.getElementById('chat-welcome-card')) return;

    if (data.current_activity) {
      focusBanner = `<div class="welcome-focus">
        <div class="dashboard-focus-indicator"></div>
        <span class="welcome-focus-text">${escapeHtml(data.current_activity.title)}</span>
        <span class="welcome-focus-time">${data.current_activity.elapsed || ''}</span>
      </div>`;
    }

    // Load activity tracking data
    let actHtml = '';
    try {
      const act = await invoke('get_activity_timeline', { date: null });
      if (act.active_minutes > 0 || act.idle_minutes > 0) {
        const totalActive = Math.round(act.active_minutes);
        const activeH = Math.floor(totalActive / 60);
        const activeM = totalActive % 60;
        const totalIdle = Math.round(act.idle_minutes + act.locked_minutes);
        const idleH = Math.floor(totalIdle / 60);
        const idleM = totalIdle % 60;
        const totalUnknown = Math.round(act.unknown_minutes || 0);
        const unknownH = Math.floor(totalUnknown / 60);
        const unknownM = totalUnknown % 60;
        actHtml = `
        <div class="welcome-stat"><div class="welcome-stat-value">${activeH}ч ${activeM}м</div><div class="welcome-stat-label">Активность</div></div>
        <div class="welcome-stat"><div class="welcome-stat-value">${idleH}ч ${idleM}м</div><div class="welcome-stat-label">AFK</div></div>
        <div class="welcome-stat"><div class="welcome-stat-value">${unknownH}ч ${unknownM}м</div><div class="welcome-stat-label">Неизвестно</div></div>`;
      }
    } catch (_) {}

    statsHtml = `
      <div class="welcome-stats">${actHtml}</div>
      <div class="welcome-stats">
        <div class="welcome-stat"><div class="welcome-stat-value">${data.focus_minutes || 0}м</div><div class="welcome-stat-label">Фокус</div></div>
        <div class="welcome-stat"><div class="welcome-stat-value">${data.notes_count || 0}</div><div class="welcome-stat-label">Заметки</div></div>
        <div class="welcome-stat"><div class="welcome-stat-value">${data.events_today || 0}</div><div class="welcome-stat-label">События</div></div>
      </div>`;

    if (data.events && data.events.length > 0) {
      eventsHtml = `<div class="welcome-events">
        <div class="welcome-section-title">Сегодня</div>
        ${data.events.slice(0, 3).map(e => `<div class="welcome-event">
          <span class="welcome-event-time">${e.time || ''}</span>
          <span class="welcome-event-title">${escapeHtml(e.title)}</span>
        </div>`).join('')}
      </div>`;
    }

    card.innerHTML = `
      <div class="welcome-greeting">${greeting}!</div>
      <div class="welcome-date">${dateStr}</div>
      ${focusBanner}${statsHtml}${eventsHtml}
      <div class="welcome-actions">
        <button class="welcome-action-btn" data-nav="notes">Заметка</button>
        <button class="welcome-action-btn" data-nav="focus">Фокус</button>
        <button class="welcome-action-btn" data-nav="calendar">Календарь</button>
        <button class="welcome-action-btn" data-nav="health">Здоровье</button>
      </div>`;

    // Re-attach navigation handlers after re-render
    card.querySelectorAll('.welcome-action-btn[data-nav]').forEach(btn => {
      btn.addEventListener('click', () => tabLoaders.switchTab(btn.dataset.nav));
    });
  } catch (_) {}
}

// ── Chat Floating Overlay ──

let coPanel = null;
let coMessages = null;
let coInput = null;
let coTyping = null;
let coFab = null;

function createChatOverlay() {
  if (document.getElementById('chat-overlay')) return;

  // FAB button
  const fab = document.createElement('button');
  fab.id = 'chat-overlay-fab';
  fab.className = 'chat-overlay-fab hidden';
  fab.title = 'Открыть чат (⌘⇧C)';
  fab.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>';
  coFab = fab;

  // Panel
  const panel = document.createElement('div');
  panel.id = 'chat-overlay';
  panel.className = 'chat-overlay hidden';
  panel.innerHTML = `
    <div class="co-header">
      <div class="co-header-left">
        <button class="co-conv-selector" id="co-conv-selector" title="Выбрать чат">
          <span class="co-conv-name" id="co-conv-name">Новый чат</span>
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </button>
        <button class="co-new-chat-btn" id="co-new-chat" title="Новый чат">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </button>
      </div>
      <div class="co-header-actions">
        <button id="co-goto-chat" title="Открыть полный чат">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>
        </button>
        <button id="co-close" title="Закрыть">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
    </div>
    <div class="co-conv-dropdown hidden" id="co-conv-dropdown"></div>
    <div class="co-messages" id="co-messages"></div>
    <div class="co-typing hidden" id="co-typing">Печатает...</div>
    <div class="co-input-area">
      <div class="co-input-wrapper">
        <textarea class="co-input" id="co-input" placeholder="Сообщение..." rows="1"></textarea>
        <button class="co-send" id="co-send">
          <svg viewBox="0 0 16 16" fill="none"><path d="M3 13V3l11 5-11 5z" fill="currentColor"/></svg>
        </button>
      </div>
    </div>
  `;

  const contentArea = document.getElementById('content-area');
  contentArea.appendChild(fab);
  contentArea.appendChild(panel);

  coPanel = panel;
  coMessages = panel.querySelector('#co-messages');
  coInput = panel.querySelector('#co-input');
  coTyping = panel.querySelector('#co-typing');

  // Events
  fab.addEventListener('click', toggleChatOverlay);

  panel.querySelector('#co-close').addEventListener('click', () => {
    coPanel.classList.add('hidden');
    coFab.classList.remove('hidden');
  });

  panel.querySelector('#co-goto-chat').addEventListener('click', () => {
    coPanel.classList.add('hidden');
    coFab.classList.add('hidden');
    tabLoaders.switchTab('chat');
  });

  // New chat button
  panel.querySelector('#co-new-chat').addEventListener('click', async () => {
    await newChat();
    updateOverlayConvName();
    refreshOverlayMessages();
  });

  // Conversation selector dropdown
  const convSelector = panel.querySelector('#co-conv-selector');
  const convDropdown = panel.querySelector('#co-conv-dropdown');
  convSelector.addEventListener('click', async (e) => {
    e.stopPropagation();
    const isOpen = !convDropdown.classList.contains('hidden');
    if (isOpen) {
      convDropdown.classList.add('hidden');
      return;
    }
    // Load conversations
    try {
      const convs = await invoke('get_conversations', { limit: 15 });
      convDropdown.innerHTML = '';
      if (!convs || convs.length === 0) {
        convDropdown.innerHTML = '<div class="co-conv-item co-conv-empty">Нет чатов</div>';
      } else {
        for (const c of convs) {
          const item = document.createElement('div');
          item.className = 'co-conv-item';
          if (c.id === S.currentConversationId) item.classList.add('active');
          item.textContent = c.title || c.summary || `Чат #${c.id}`;
          item.addEventListener('click', async () => {
            convDropdown.classList.add('hidden');
            await tabLoaders.loadConversation(c.id);
            updateOverlayConvName();
            refreshOverlayMessages();
          });
          convDropdown.appendChild(item);
        }
      }
      convDropdown.classList.remove('hidden');
    } catch (_) {}
  });

  const coSendBtn = panel.querySelector('#co-send');
  coSendBtn.addEventListener('click', sendFromOverlay);

  coInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendFromOverlay();
    }
  });

  // Auto-resize textarea
  coInput.addEventListener('input', () => {
    coInput.style.height = 'auto';
    coInput.style.height = Math.min(coInput.scrollHeight, 80) + 'px';
  });

  // Close on outside click (also close dropdown)
  document.addEventListener('click', (e) => {
    if (!coPanel.classList.contains('hidden') && !coPanel.contains(e.target) && !coFab.contains(e.target)) {
      coPanel.classList.add('hidden');
      coFab.classList.remove('hidden');
    }
    // Close dropdown if clicking outside it
    if (convDropdown && !convDropdown.classList.contains('hidden') && !convSelector.contains(e.target) && !convDropdown.contains(e.target)) {
      convDropdown.classList.add('hidden');
    }
  });
}

function updateOverlayConvName() {
  const nameEl = document.getElementById('co-conv-name');
  if (!nameEl) return;
  if (!S.currentConversationId) {
    nameEl.textContent = 'Новый чат';
  } else {
    // Try to get title from first user message
    const firstUser = S.history.find(m => getRole(m) === 'user');
    const content = firstUser ? getContent(firstUser) : '';
    nameEl.textContent = content ? (content.length > 30 ? content.substring(0, 30) + '...' : content) : `Чат #${S.currentConversationId}`;
  }
}

function toggleChatOverlay() {
  if (!coPanel) return;
  const isHidden = coPanel.classList.contains('hidden');
  if (isHidden) {
    updateOverlayConvName();
    refreshOverlayMessages();
    coPanel.classList.remove('hidden');
    coFab.classList.add('hidden');
    coInput.focus();
  } else {
    coPanel.classList.add('hidden');
    coFab.classList.remove('hidden');
  }
}

function refreshOverlayMessages() {
  if (!coMessages) return;
  coMessages.innerHTML = '';

  // Show last 20 messages from history
  const msgs = S.history.slice(-20);
  if (msgs.length === 0) {
    coMessages.innerHTML = '<div class="co-empty">Начните разговор</div>';
    return;
  }

  for (const m of msgs) {
    const role = getRole(m);
    if (role === 'tool' || role === 'system') continue;
    const content = getContent(m);
    if (!content) continue;

    const div = document.createElement('div');
    if (role === 'user') {
      div.className = 'co-msg user';
      div.textContent = content;
    } else {
      div.className = 'co-msg bot markdown-body';
      div.innerHTML = renderMarkdown(content);
    }
    coMessages.appendChild(div);
  }

  coMessages.scrollTop = coMessages.scrollHeight;
}

async function sendFromOverlay() {
  const text = coInput.value.trim();
  if (!text || S.busy) return;

  // Add user message to overlay immediately
  const userDiv = document.createElement('div');
  userDiv.className = 'co-msg user';
  userDiv.textContent = text;
  coMessages.querySelector('.co-empty')?.remove();
  coMessages.appendChild(userDiv);
  coMessages.scrollTop = coMessages.scrollHeight;

  // Show typing indicator
  coTyping.classList.remove('hidden');

  // Send via main chat
  coInput.value = '';
  coInput.style.height = 'auto';
  input.value = text;
  await send();

  // Refresh overlay with actual response
  coTyping.classList.add('hidden');
  refreshOverlayMessages();
}

function updateChatOverlayVisibility() {
  if (!coFab) return;
  const onChatTab = S.activeTab === 'chat';
  if (onChatTab) {
    coFab.classList.add('hidden');
    coPanel?.classList.add('hidden');
  } else {
    // Show FAB only when overlay panel is closed
    if (coPanel?.classList.contains('hidden')) {
      coFab.classList.remove('hidden');
    }
  }
}

// Listen for new proactive messages to refresh overlay
listen('proactive-message', () => {
  if (coPanel && !coPanel.classList.contains('hidden')) {
    setTimeout(refreshOverlayMessages, 100);
  }
});

// ── Exports ──

export {
  addMsg,
  scrollDown,
  send,
  newChat,
  renderChatWelcomeCard,
  removeChatWelcomeCard,
  showChatSettingsMode,
  hideChatSettingsMode,
  loadChatSettings,
  addFeedbackButtons,
  addProactiveFeedbackButtons,
  showStub,
  streamChat,
  toggleTTS,
  stopAllTTS,
  showAgentIndicator,
  createChatOverlay,
  toggleChatOverlay,
  updateChatOverlayVisibility,
  refreshOverlayMessages,
};
