// ── js/chat-render.js — Message rendering, bubbles, feedback buttons, welcome card, TTS, scroll ──

import { S, invoke, chat, tabLoaders } from './state.js';
import { renderMarkdown, escapeHtml } from './utils.js';

// ── Scroll helpers ──

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

// ── Add message to chat ──

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

// ── Feedback buttons ──

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
    tabLoaders.send();
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

function addProactiveFeedbackButtons(wrapper, proactiveId, botText) {
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
        await invoke('rate_proactive', { proactiveId, rating });
      } catch (e) {
        console.error('Rate proactive error:', e);
      }
    } else {
      try {
        await invoke('rate_proactive', { proactiveId, rating: 0 });
      } catch (e) {
        console.error('Rate proactive error:', e);
      }
    }
  };

  thumbUp.addEventListener('click', () => handleClick(thumbUp, 1));
  thumbDown.addEventListener('click', () => handleClick(thumbDown, -1));

  actions.appendChild(copyBtn);
  actions.appendChild(thumbUp);
  actions.appendChild(thumbDown);
  wrapper.appendChild(actions);
}

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
    ${focusBanner}${statsHtml}${eventsHtml}`;
  chat.appendChild(card);

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

    statsHtml = `
      <div class="welcome-stats">
        <div class="welcome-stat"><div class="welcome-stat-value">${data.activities_today || 0}</div><div class="welcome-stat-label">Активности</div></div>
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

    // Load activity timeline
    let activityHtml = '';
    try {
      const activity = await invoke('get_activity_timeline', { date: null });
      if (activity && activity.snapshots_count > 0) {
        const catColors = {
          coding: 'var(--accent-blue)', writing: 'var(--accent-purple)',
          learning: 'var(--accent-teal)', browsing: 'var(--accent-yellow)',
          social: 'var(--accent-orange)', media: 'var(--accent-red)',
          communication: 'var(--accent-green)', other: 'var(--text-muted)',
        };
        const catLabels = {
          coding: 'Код', writing: 'Текст', learning: 'Обучение',
          browsing: 'Браузер', social: 'Соцсети', media: 'Медиа',
          communication: 'Общение', other: 'Другое',
        };

        const cats = activity.categories || {};
        const totalMin = activity.total_minutes || 1;
        const totalHrs = Math.floor(totalMin / 60);
        const totalM = Math.round(totalMin % 60);
        const timeHeader = totalHrs > 0 ? `${totalHrs}ч ${totalM}м` : `${totalM}м`;

        const catBars = Object.entries(cats)
          .sort((a, b) => b[1] - a[1])
          .map(([cat, min]) => {
            const pct = Math.max(2, (min / totalMin) * 100);
            const hrs = Math.floor(min / 60);
            const mins = Math.round(min % 60);
            const timeStr = hrs > 0 ? `${hrs}ч ${mins}м` : `${mins}м`;
            return `<div class="act-cat-row">
              <span class="act-cat-label" style="color:${catColors[cat] || catColors.other}">${catLabels[cat] || cat}</span>
              <div class="act-cat-bar-bg"><div class="act-cat-bar" style="width:${pct}%;background:${catColors[cat] || catColors.other}"></div></div>
              <span class="act-cat-time">${timeStr}</span>
            </div>`;
          }).join('');

        activityHtml = `
          <div class="act-header">
            <span class="welcome-section-title">Экранное время</span>
            <span class="act-total-time">${timeHeader}</span>
          </div>
          <div class="act-categories">${catBars}</div>`;
      }
    } catch (_) {}

    card.innerHTML = `
      <div class="welcome-greeting">${greeting}!</div>
      <div class="welcome-date">${dateStr}</div>
      ${focusBanner}${statsHtml}${eventsHtml}${activityHtml}`;
  } catch (_) {}
}

export {
  scrollDown,
  addMsg,
  addFeedbackButtons,
  addProactiveFeedbackButtons,
  showAgentIndicator,
  toggleTTS,
  stopAllTTS,
  showStub,
  removeChatWelcomeCard,
  renderChatWelcomeCard,
};
