// ── js/voice.js — Voice recording, STT, wake word SSE, and call mode ──

import { S, invoke, listen, emit, chat, input, sendBtn, recordBtn, callBtn, callOverlay, callPhaseText, callTranscriptArea, callEndBtn, callWaveform, callStatusHint, callDurationEl, callWaveBars, tabLoaders, VOICE_SERVER, PHASE_LABELS, MAX_TRANSCRIPT_CHILDREN } from './state.js';
import { escapeHtml, renderMarkdown, confirmModal, getRole } from './utils.js';
import { autoSaveConversation, loadConversationsList } from './conversations.js';
import { parseAndExecuteActions, executeAction } from './actions.js';

// ── Voice Server Health Check ──

async function checkVoiceServer(retries = 1) {
  for (let i = 0; i < retries; i++) {
    try {
      const r = await fetch(`${VOICE_SERVER}/health`, { signal: AbortSignal.timeout(2000) });
      if (r.ok) { S.voiceServerAvailable = true; return true; }
    } catch (_) {}
    if (i < retries - 1) await new Promise(r => setTimeout(r, 2000));
  }
  S.voiceServerAvailable = false;
  return false;
}
// Check on startup with retries (voice server may take time to start)
setTimeout(() => checkVoiceServer(5), 3000);

// ── Voice Recording ──

async function startRecording() {
  if (S.isRecording || S.busy) return;
  // Block proactive messages while recording
  invoke('set_recording_state', { recording: true }).catch(() => {});
  invoke('set_user_typing', { typing: true }).catch(() => {});
  await checkVoiceServer();

  if (S.voiceServerAvailable) {
    S.isRecording = true;
    S.voiceRecordStartTime = performance.now();
    recordBtn.classList.add('recording');
    recordBtn.title = 'Отпустите для отправки';
    // Start recording (blocks until silence or /finish)
    S.recordPending = fetch(`${VOICE_SERVER}/transcribe`, { method: 'POST' })
      .then(r => r.json())
      .catch(() => null);
  } else {
    // Fallback: Rust cpal + whisper-rs
    try {
      const hasModel = await invoke('check_whisper_model');
      if (!hasModel) {
        if (await confirmModal('Модель Whisper не найдена (~1.5GB). Скачать?')) {
          tabLoaders.addMsg('bot', 'Скачиваю модель Whisper...');
          const unlisten = await listen('whisper-download-progress', (event) => {
            const msgs = chat.querySelectorAll('.msg.bot');
            const last = msgs[msgs.length - 1];
            if (last) last.textContent = `Скачиваю Whisper... ${event.payload}%`;
          });
          try { await invoke('download_whisper_model'); tabLoaders.addMsg('bot', 'Whisper загружен!'); } catch (e) { tabLoaders.addMsg('bot', 'Ошибка: ' + e); }
          unlisten();
        }
        return;
      }
      await invoke('start_recording');
      S.isRecording = true;
      recordBtn.classList.add('recording');
      recordBtn.title = 'Отпустите для отправки';
    } catch (e) { tabLoaders.addMsg('bot', 'Ошибка записи: ' + e); }
  }
}

async function stopRecordingAndSend() {
  if (!S.isRecording) return;
  S.isRecording = false;
  recordBtn.classList.remove('recording');
  recordBtn.classList.add('transcribing');
  recordBtn.title = 'Распознаю...';
  recordBtn.disabled = true;

  if (S.voiceServerAvailable && S.recordPending) {
    // Signal voice server to finish recording (triggers transcription of collected audio)
    try { await fetch(`${VOICE_SERVER}/finish`, { method: 'POST' }); } catch (_) {}
    const data = await S.recordPending;
    S.recordPending = null;
    if (data && data.text && data.text.trim()) {
      S.lastMessageWasVoice = true;
      S.lastSttTimeMs = performance.now() - S.voiceRecordStartTime;
      input.value = (input.value ? input.value + ' ' : '') + data.text.trim();
      sendBtn.click();
    }
  } else {
    // Fallback: Rust whisper-rs
    try {
      const text = await invoke('stop_recording');
      if (text && text.trim()) {
        S.lastMessageWasVoice = true;
        S.lastSttTimeMs = performance.now() - S.voiceRecordStartTime;
        input.value = (input.value ? input.value + ' ' : '') + text.trim();
        sendBtn.click();
      }
    } catch (e) {
      if (!String(e).includes('No audio')) tabLoaders.addMsg('bot', 'Ошибка: ' + e);
    }
  }
  recordBtn.classList.remove('transcribing');
  recordBtn.disabled = false;
  recordBtn.title = 'Удерживайте для записи';
  // Release recording lock immediately
  invoke('set_recording_state', { recording: false }).catch(() => {});
  // Release typing lock (delayed — give send() time to acquire busy flag)
  setTimeout(() => invoke('set_user_typing', { typing: false }).catch(() => {}), 3000);
}

function cancelRecording() {
  if (!S.isRecording) return;
  S.isRecording = false;
  S.recordPending = null;
  recordBtn.classList.remove('recording');
  recordBtn.title = 'Удерживайте для записи';
  invoke('set_recording_state', { recording: false }).catch(() => {});
  invoke('set_user_typing', { typing: false }).catch(() => {});
  if (S.voiceServerAvailable) {
    fetch(`${VOICE_SERVER}/stop`, { method: 'POST' }).catch(() => {});
  } else {
    invoke('stop_recording').catch(() => {});
  }
}

// Press-and-hold handlers
recordBtn.addEventListener('mousedown', (e) => { e.preventDefault(); startRecording(); });
recordBtn.addEventListener('mouseup', () => stopRecordingAndSend());
recordBtn.addEventListener('mouseleave', () => { if (S.isRecording) cancelRecording(); });
recordBtn.addEventListener('touchstart', (e) => { e.preventDefault(); startRecording(); }, { passive: false });
recordBtn.addEventListener('touchend', (e) => { e.preventDefault(); stopRecordingAndSend(); });

// Cancel recording with Escape
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && S.isRecording) {
    e.preventDefault();
    cancelRecording();
  }
});

// ── Wake Word SSE Management ──

function startWakeWordSSE(keyword) {
  stopWakeWordSSE();
  if (!S.voiceServerAvailable) return;
  try {
    S._wakeWordSSE = new EventSource(`${VOICE_SERVER}/wakeword/events`);
    S._wakeWordSSE.onmessage = async (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.detected && !S.callModeActive) {
          console.log('[WakeWord] Detected! Starting call mode...');
          // Stop wake word (call mode will use mic)
          stopWakeWordSSE();
          try { await fetch(`${VOICE_SERVER}/wakeword/stop`, { method: 'POST' }); } catch (_) {}
          // Focus window and start call mode
          try {
            const { getCurrentWindow } = window.__TAURI__.window;
            const win = getCurrentWindow();
            await win.unminimize();
            await win.show();
            await win.setFocus();
          } catch (_) {}
          toggleCallMode();
        }
      } catch (_) {}
    };
    S._wakeWordSSE.onerror = () => {
      // SSE disconnected — will reconnect or stop
      stopWakeWordSSE();
    };
  } catch (_) {}
}

function stopWakeWordSSE() {
  if (S._wakeWordSSE) {
    S._wakeWordSSE.close();
    S._wakeWordSSE = null;
  }
}

// Auto-start wake word on load if enabled
(async () => {
  try {
    const enabled = await invoke('get_app_setting', { key: 'wakeword_enabled' });
    if (enabled === 'true' && S.voiceServerAvailable !== false) {
      const keyword = await invoke('get_app_setting', { key: 'wakeword_keyword' }).catch(() => 'ханни');
      // Wait for voice server to be ready
      setTimeout(async () => {
        if (!S.voiceServerAvailable) return;
        try {
          await fetch(`${VOICE_SERVER}/wakeword/start`, {
            method: 'POST', headers: {'Content-Type':'application/json'},
            body: JSON.stringify({ keyword: keyword || 'ханни' }),
          });
          startWakeWordSSE(keyword || 'ханни');
        } catch (_) {}
      }, 8000); // Wait for voice server boot
    }
  } catch (_) {}
})();

// ── Call Mode ──

async function toggleCallMode() {
  if (S.callModeActive) {
    await endCallMode();
  } else {
    await startCallMode();
  }
}

async function startCallMode() {
  if (S.callInitializing) return;  // prevent double-init
  S.callInitializing = true;
  try {
  S.callModeActive = true;
  callBtn.classList.add('active');
  callOverlay.classList.remove('hidden');
  callOverlay.setAttribute('data-phase', 'listening');
  callPhaseText.textContent = PHASE_LABELS.listening;
  callTranscriptArea.innerHTML = '';
  if (callStatusHint) callStatusHint.textContent = '';

  // Start wave observer + call duration timer
  ensureWaveObserver();
  S.callStartTime = Date.now();
  if (callDurationEl) callDurationEl.textContent = '0:00';
  S.callDurationInterval = setInterval(() => {
    const total = Math.floor((Date.now() - S.callStartTime) / 1000);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (callDurationEl) callDurationEl.textContent = h > 0
      ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
      : `${m}:${s.toString().padStart(2, '0')}`;
  }, 1000);

  // Start a fresh chat for this call
  await autoSaveConversation();
  S.currentConversationId = null;
  S.history = [];
  chat.innerHTML = '';
  tabLoaders.addMsg('bot', 'Звонок начат... Говорите!');

  // Disable normal input
  input.disabled = true;
  sendBtn.disabled = true;
  recordBtn.disabled = true;

  await checkVoiceServer();

  if (S.voiceServerAvailable) {
    // Use Python voice server for call mode (SSE stream)
    // Python voice server has its own MLX Whisper — no Rust model needed
    try {
      // Close any stale EventSource from a previous session
      if (window._callEventSource) {
        window._callEventSource.close();
        window._callEventSource = null;
      }

      const eventSource = new EventSource(`${VOICE_SERVER}/listen`);
      window._callEventSource = eventSource;

      eventSource.onmessage = (event) => {
        if (!S.callModeActive) { eventSource.close(); window._callEventSource = null; return; }
        try {
          const data = JSON.parse(event.data);
          if (data.error) {
            tabLoaders.addMsg('bot', 'Ошибка голоса: ' + data.error);
            endCallMode();
            return;
          }
          if (data.phase === 'listening') {
            callOverlay.setAttribute('data-phase', 'listening');
            callPhaseText.textContent = PHASE_LABELS.listening;
            return;
          }
          if (data.text) {
            const customEvent = new CustomEvent('voice-server-transcript', {
              detail: { text: data.text, sttMs: data.stt_ms || 0 }
            });
            window.dispatchEvent(customEvent);
          }
        } catch (_) {}
      };

      let sseRetryCount = 0;
      eventSource.onerror = () => {
        if (!S.callModeActive) { eventSource.close(); window._callEventSource = null; return; }
        sseRetryCount++;
        if (sseRetryCount >= 3) {
          eventSource.close();
          window._callEventSource = null;
          tabLoaders.addMsg('bot', 'Голосовой сервер недоступен — переключаюсь на Rust режим');
          // Fallback to Rust call mode
          (async () => {
            if (!S.callModeActive) return;  // abort if call already ended
            try {
              const hasModel = await invoke('check_whisper_model');
              if (!S.callModeActive) return;
              if (hasModel) {
                await invoke('start_call_mode');
              } else {
                tabLoaders.addMsg('bot', 'Rust Whisper модель не установлена');
                await endCallMode();
              }
            } catch (e) {
              if (!S.callModeActive) return;
              tabLoaders.addMsg('bot', 'Ошибка запуска Rust режима: ' + e);
              await endCallMode();
            }
          })();
        }
      };
    } catch (e) {
      window._callEventSource = null;
      tabLoaders.addMsg('bot', 'Ошибка голосового сервера: ' + e);
      await endCallMode();
    }
  } else {
    // Fallback: Rust call mode (needs Rust Whisper ggml model)
    try {
      const hasModel = await invoke('check_whisper_model');
      if (!hasModel) {
        if (await confirmModal('Модель Whisper не найдена (~1.5GB). Скачать для голосового ввода?')) {
          tabLoaders.addMsg('bot', 'Скачиваю модель Whisper...');
          const unlisten = await listen('whisper-download-progress', (event) => {
            const msgs = chat.querySelectorAll('.msg.bot');
            const last = msgs[msgs.length - 1];
            if (last) last.textContent = `Скачиваю Whisper... ${event.payload}%`;
          });
          try {
            await invoke('download_whisper_model');
            tabLoaders.addMsg('bot', 'Whisper загружен!');
          } catch (e) {
            tabLoaders.addMsg('bot', 'Ошибка загрузки Whisper: ' + e);
            unlisten();
            await endCallMode();
            return;
          }
          unlisten();
        } else {
          await endCallMode();
          return;
        }
      }
    } catch (e) {
      tabLoaders.addMsg('bot', 'Ошибка Whisper: ' + e);
      await endCallMode();
      return;
    }
    try {
      await invoke('start_call_mode');
    } catch (e) {
      tabLoaders.addMsg('bot', 'Ошибка запуска звонка: ' + e);
      await endCallMode();
    }
  }
  } finally {
    S.callInitializing = false;
  }
}

async function endCallMode() {
  S.callModeActive = false;
  S.callInitializing = false;
  S.callBusy = false;
  S.callPendingTranscript = null;
  callBtn.classList.remove('active');
  callOverlay.classList.add('hidden');

  // Stop call duration timer + wave observer
  if (S.callDurationInterval) { clearInterval(S.callDurationInterval); S.callDurationInterval = null; }
  if (S.callWaveObserver) { S.callWaveObserver.disconnect(); S.callWaveObserver = null; }
  if (S.ambientWaveFrame) { cancelAnimationFrame(S.ambientWaveFrame); S.ambientWaveFrame = null; }

  // Reset waveform
  for (const bar of callWaveBars) bar.style.height = '6px';
  if (callStatusHint) callStatusHint.textContent = '';

  // Close voice server SSE if active
  if (window._callEventSource) {
    window._callEventSource.close();
    window._callEventSource = null;
    try { await fetch(`${VOICE_SERVER}/listen/stop`, { method: 'POST' }); } catch (_) {}
    // Cooldown: let Python server release _listen_lock before allowing restart
    await new Promise(r => setTimeout(r, 150));
  }

  // Re-enable input
  input.disabled = false;
  sendBtn.disabled = false;
  recordBtn.disabled = false;

  try {
    await invoke('stop_call_mode');
  } catch (e) {
    console.error('[call] stop_call_mode failed:', e);
  }

  // Resume wake word if it was enabled
  (async () => {
    try {
      const wkEnabled = await invoke('get_app_setting', { key: 'wakeword_enabled' });
      if (wkEnabled === 'true') {
        const wkKeyword = await invoke('get_app_setting', { key: 'wakeword_keyword' }).catch(() => 'ханни');
        await fetch(`${VOICE_SERVER}/wakeword/start`, {
          method: 'POST', headers: {'Content-Type':'application/json'},
          body: JSON.stringify({ keyword: wkKeyword || 'ханни' }),
        });
        startWakeWordSSE(wkKeyword || 'ханни');
      }
    } catch (_) {}
  })();

  input.focus();
}

// Listen for phase changes
listen('call-phase-changed', (event) => {
  const phase = event.payload;
  if (!S.callModeActive && phase !== 'idle') return;
  callOverlay.setAttribute('data-phase', phase);
  callPhaseText.textContent = PHASE_LABELS[phase] || phase;
});

// Audio level visualization (waveform bars)
listen('call-audio-level', (event) => {
  if (!S.callModeActive || !callWaveBars.length) return;
  const level = event.payload; // 0-100
  const barCount = callWaveBars.length;
  const center = Math.floor(barCount / 2);
  for (let i = 0; i < barCount; i++) {
    const dist = Math.abs(i - center);
    const scale = Math.max(0, 1 - dist * 0.15);
    const jitter = 0.7 + Math.random() * 0.6;
    const h = Math.max(6, (level / 100) * 34 * scale * jitter);
    callWaveBars[i].style.height = h + 'px';
  }
});

// Ambient waveform animation for processing/speaking phases (no real audio level)
function animateAmbientWave() {
  if (!S.callModeActive || !callWaveBars.length) { S.ambientWaveFrame = null; return; }
  const phase = callOverlay.getAttribute('data-phase');
  if (phase !== 'processing' && phase !== 'speaking') { S.ambientWaveFrame = null; return; }
  const t = Date.now() / 1000;
  const amplitude = phase === 'speaking' ? 20 : 12;
  const speed = phase === 'speaking' ? 3 : 2;
  const barCount = callWaveBars.length;
  for (let i = 0; i < barCount; i++) {
    const h = Math.max(6, amplitude * (0.5 + 0.5 * Math.sin(t * speed + i * 0.8)) + Math.random() * 4);
    callWaveBars[i].style.height = h + 'px';
  }
  S.ambientWaveFrame = requestAnimationFrame(animateAmbientWave);
}

// Start ambient wave when phase changes to processing/speaking
function ensureWaveObserver() {
  if (S.callWaveObserver) return;
  S.callWaveObserver = new MutationObserver(() => {
    const phase = callOverlay.getAttribute('data-phase');
    if ((phase === 'processing' || phase === 'speaking') && !S.ambientWaveFrame) {
      animateAmbientWave();
    }
  });
  S.callWaveObserver.observe(callOverlay, { attributes: true, attributeFilter: ['data-phase'] });
}

// Limit transcript DOM to last N elements to prevent memory growth on long calls
function trimTranscript() {
  while (callTranscriptArea.children.length > MAX_TRANSCRIPT_CHILDREN) {
    callTranscriptArea.removeChild(callTranscriptArea.firstChild);
  }
}

// Not-heard feedback
listen('call-not-heard', (event) => {
  if (!S.callModeActive || !callStatusHint) return;
  callStatusHint.textContent = 'Не расслышала, повторите...';
  callStatusHint.classList.remove('flash');
  void callStatusHint.offsetWidth; // force reflow
  callStatusHint.classList.add('flash');
});

// Barge-in visual feedback
listen('call-barge-in', () => {
  if (!S.callModeActive) return;
  // Flash the overlay border
  callOverlay.classList.remove('barged-in');
  void callOverlay.offsetWidth;
  callOverlay.classList.add('barged-in');
  setTimeout(() => callOverlay.classList.remove('barged-in'), 600);
  if (callStatusHint) {
    callStatusHint.textContent = 'Перебили — слушаю!';
    callStatusHint.classList.remove('flash');
    void callStatusHint.offsetWidth;
    callStatusHint.classList.add('flash');
  }
});

// Audio error — auto end call
listen('call-error', async (event) => {
  if (!S.callModeActive) return;
  tabLoaders.addMsg('bot', event.payload || 'Ошибка аудио');
  await endCallMode();
});

// Handle transcripts from Python voice server
window.addEventListener('voice-server-transcript', (event) => {
  if (!S.callModeActive || !event.detail) return;
  const { text, sttMs } = event.detail;
  handleCallTranscript(text, sttMs || 0);
});

// Listen for transcripts from Rust call mode
listen('call-transcript', async (event) => {
  if (!S.callModeActive || !event.payload) return;
  handleCallTranscript(event.payload, 0);
});

async function handleCallTranscript(userText, sttMs = 0) {
  if (!S.callModeActive || !userText) return;

  // Guard: if LLM is already processing, queue the latest transcript
  if (S.callBusy) {
    S.callPendingTranscript = { text: userText, sttMs };
    return;
  }
  S.callBusy = true;

  // Pause voice server mic during LLM processing to avoid new transcripts
  const useVoiceServer = !!window._callEventSource;
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/pause`, { method: 'POST' }); } catch (_) {}
  }

  // Show user bubble in overlay
  const userBubble = document.createElement('div');
  userBubble.className = 'call-transcript-user';
  userBubble.textContent = userText;
  callTranscriptArea.appendChild(userBubble);
  trimTranscript();
  callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;

  // Also add to actual chat history (voice)
  tabLoaders.addMsg('user', userText, true);
  S.history.push({ role: 'user', content: userText });
  { const lastUserWrapper = chat.querySelector('.user-wrapper:last-of-type');
    if (lastUserWrapper) lastUserWrapper.dataset.historyIdx = String(S.history.length - 1); }

  // Update UI phase to "processing" while LLM thinks
  callOverlay.setAttribute('data-phase', 'processing');
  callPhaseText.textContent = PHASE_LABELS.processing;

  // Run LLM — same agentic loop as send()
  try {
  const t0 = performance.now();
  let iteration = 0;
  const MAX_ITERATIONS = 5;
  let lastReply = '';

  while (iteration < MAX_ITERATIONS) {
    iteration++;
    if (iteration > 1) tabLoaders.showAgentIndicator(iteration);

    const wrapper = document.createElement('div');
    wrapper.className = 'msg-wrapper';
    const botDiv = document.createElement('div');
    botDiv.className = 'msg bot';
    wrapper.appendChild(botDiv);
    chat.appendChild(wrapper);
    tabLoaders.scrollDown();

    const result = await tabLoaders.streamChat(botDiv, t0, true);

    // Primary path: native tool calls
    if (result.toolCalls && result.toolCalls.length > 0) {
      S.history.push({ role: 'assistant', content: result.fullReply || null, tool_calls: result.toolCalls });
      wrapper.dataset.historyIdx = String(S.history.length - 1);
      lastReply = result.fullReply || '';
      botDiv.classList.add('intermediate');

      for (const tc of result.toolCalls) {
        let args;
        try { args = JSON.parse(tc.function.arguments); } catch (_) { args = {}; }
        args.action = tc.function.name;
        const actionJson = JSON.stringify(args);
        const { success, result: actionResult } = await executeAction(actionJson);
        const actionDiv = document.createElement('div');
        actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
        actionDiv.textContent = actionResult;
        chat.appendChild(actionDiv);
        tabLoaders.scrollDown();
        // Show action result in call overlay
        if (S.callModeActive) {
          const callAction = document.createElement('div');
          callAction.className = `call-action-result ${success ? 'success' : 'error'}`;
          callAction.textContent = `${success ? '\u2713' : '\u2717'} ${actionResult}`;
          callTranscriptArea.appendChild(callAction);
          callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
        }
        S.history.push({ role: 'tool', tool_call_id: tc.id, name: tc.function.name, content: String(actionResult) });
      }
      continue;
    }

    if (!result.fullReply) break;

    S.history.push({ role: 'assistant', content: result.fullReply });
    wrapper.dataset.historyIdx = String(S.history.length - 1);
    lastReply = result.fullReply;

    // Fallback: parse ```action blocks
    const actions = parseAndExecuteActions(result.fullReply);
    if (actions.length === 0) break;

    botDiv.classList.add('intermediate');
    const results = [];
    for (const actionJson of actions) {
      const { success, result: actionResult } = await executeAction(actionJson);
      const actionDiv = document.createElement('div');
      actionDiv.className = `action-result ${success ? 'success' : 'error'}`;
      actionDiv.textContent = actionResult;
      chat.appendChild(actionDiv);
      tabLoaders.scrollDown();
      // Show action result in call overlay
      if (S.callModeActive) {
        const callAction = document.createElement('div');
        callAction.className = `call-action-result ${success ? 'success' : 'error'}`;
        callAction.textContent = `${success ? '\u2713' : '\u2717'} ${actionResult}`;
        callTranscriptArea.appendChild(callAction);
        callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
      }
      results.push(actionResult);
    }
    S.history.push({ role: 'user', content: `[Action result: ${results.join('; ')}]` });
  }

  if (!S.callModeActive) return;

  const llmMs = performance.now() - t0;

  // Show bot reply + timing in overlay
  if (lastReply) {
    const displayText = lastReply
      .replace(/```action[\s\S]*?```/g, '')
      .replace(/<think>[\s\S]*?<\/think>/g, '')
      .trim();
    if (displayText) {
      const botBubble = document.createElement('div');
      botBubble.className = 'call-transcript-bot';
      botBubble.textContent = displayText;
      callTranscriptArea.appendChild(botBubble);

      // Show latency breakdown in call overlay
      const parts = [];
      if (sttMs > 0) parts.push(`STT ${(sttMs / 1000).toFixed(1)}s`);
      parts.push(`LLM ${(llmMs / 1000).toFixed(1)}s`);
      const callTiming = document.createElement('div');
      callTiming.className = 'call-timing';
      callTiming.textContent = parts.join(' \u00b7 ');
      callTranscriptArea.appendChild(callTiming);
      trimTranscript();
      callTranscriptArea.scrollTop = callTranscriptArea.scrollHeight;
    }
  }

  // Save conversation (non-blocking) + add feedback buttons
  (async () => {
    try {
      if (S.currentConversationId) {
        await invoke('update_conversation', { id: S.currentConversationId, messages: S.history });
      } else {
        S.currentConversationId = await invoke('save_conversation', { messages: S.history });
      }
      if (S.currentConversationId) {
        chat.querySelectorAll('.msg-wrapper[data-history-idx]').forEach(w => {
          if (w.querySelector('.feedback-btn')) return;
          const idx = parseInt(w.dataset.historyIdx, 10);
          if (!isNaN(idx) && getRole(S.history[idx]) === 'assistant') {
            tabLoaders.addFeedbackButtons(w, S.currentConversationId, idx, S.history[idx]?.content || '');
          }
        });
      }
      if (S.history.length >= 2) {
        await invoke('process_conversation_end', { messages: S.history, conversationId: S.currentConversationId });
      }
      loadConversationsList();
    } catch (_) {}
  })();

  // Speak the reply sentence-by-sentence, then resume listening
  if (lastReply && S.callModeActive) {
    const ttsT0 = performance.now();
    await speakAndListen(lastReply);
    // Update timing with TTS duration
    const ttsMs = performance.now() - ttsT0;
    const timingEl = callTranscriptArea.querySelector('.call-timing:last-of-type');
    if (timingEl && ttsMs > 500) {
      timingEl.textContent += ` \u00b7 TTS ${(ttsMs / 1000).toFixed(1)}s`;
    }
  } else if (S.callModeActive) {
    // Resume voice server mic (was paused at start of handleCallTranscript)
    if (useVoiceServer) {
      try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
    }
    await invoke('call_mode_resume_listening').catch(() => {});
  }

  } finally {
    S.callBusy = false;
    // Process queued transcript (only keep the latest one)
    if (S.callPendingTranscript && S.callModeActive) {
      const pending = S.callPendingTranscript;
      S.callPendingTranscript = null;
      handleCallTranscript(pending.text, pending.sttMs);
    }
  }
}

/// Split text into sentences for streaming TTS
function splitIntoSentences(text) {
  // Split on sentence-ending punctuation, keeping the punctuation attached
  const parts = text.match(/[^.!?\u2026]+[.!?\u2026]+|[^.!?\u2026]+$/g);
  if (!parts) return [text];
  return parts.map(s => s.trim()).filter(s => s.length > 0);
}

async function speakAndListen(text) {
  if (!S.callModeActive) return;

  const useVoiceServer = !!window._callEventSource;

  // Pause voice server mic to prevent echo (TTS audio picked up by mic)
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/pause`, { method: 'POST' }); } catch (_) {}
  }

  // Set phase to speaking
  callOverlay.setAttribute('data-phase', 'speaking');
  callPhaseText.textContent = PHASE_LABELS.speaking;
  if (callStatusHint) callStatusHint.textContent = 'Можешь перебить';
  if (!useVoiceServer) await invoke('call_mode_set_speaking').catch(() => {});

  // Get voice
  let voice = 'xenia';
  try {
    const ps = await invoke('get_proactive_settings');
    voice = ps.voice_name || voice;
  } catch (_) {}

  // Strip action blocks and think blocks for TTS
  const ttsText = text
    .replace(/```action[\s\S]*?```/g, '')
    .replace(/<think>[\s\S]*?<\/think>/g, '')
    .trim();
  if (!ttsText) {
    if (useVoiceServer) {
      try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
    } else {
      await invoke('call_mode_resume_listening').catch(() => {});
    }
    if (S.callModeActive) {
      callOverlay.setAttribute('data-phase', 'listening');
      callPhaseText.textContent = PHASE_LABELS.listening;
    }
    return;
  }

  // Split into sentences for streaming TTS
  const sentences = splitIntoSentences(ttsText);

  // Barge-in polling only for Rust call mode (Rust audio loop detects speech during TTS)
  // Voice server mode: mic is paused during TTS, so no barge-in detection possible
  let bargedIn = false;
  let bargeInterval;
  if (!useVoiceServer) {
    bargeInterval = setInterval(async () => {
      if (!S.callModeActive) { clearInterval(bargeInterval); return; }
      try {
        const b = await invoke('call_mode_check_bargein');
        if (b) {
          bargedIn = true;
          clearInterval(bargeInterval);
          await invoke('stop_speaking').catch(() => {});
          if (S.callModeActive) {
            await invoke('call_mode_resume_listening').catch(() => {});
          }
        }
      } catch (_) {}
    }, 250);
  }

  // Speak each sentence sequentially
  try {
    for (const sentence of sentences) {
      if (bargedIn || !S.callModeActive) break;
      try {
        await invoke('speak_sentence_blocking', { sentence, voice });
      } catch (_) {}
      // Check barge-in between sentences (Rust mode only)
      if (!useVoiceServer && !bargedIn && S.callModeActive) {
        try {
          const b = await invoke('call_mode_check_bargein');
          if (b) {
            bargedIn = true;
            await invoke('stop_speaking').catch(() => {});
            if (S.callModeActive) {
              await invoke('call_mode_resume_listening').catch(() => {});
            }
          }
        } catch (_) {}
      }
    }
  } finally {
    if (bargeInterval) clearInterval(bargeInterval);
  }

  // Resume voice server mic after TTS finishes
  if (useVoiceServer) {
    try { await fetch(`${VOICE_SERVER}/listen/resume`, { method: 'POST' }); } catch (_) {}
  }

  // Resume listening
  if (callStatusHint) callStatusHint.textContent = '';
  if (!bargedIn && S.callModeActive) {
    callOverlay.setAttribute('data-phase', 'listening');
    callPhaseText.textContent = PHASE_LABELS.listening;
    if (!useVoiceServer) await invoke('call_mode_resume_listening').catch(() => {});
  }
}

// ── Call Mode Button Handlers ──

callBtn.addEventListener('click', toggleCallMode);
callEndBtn.addEventListener('click', endCallMode);

// Global shortcut: Cmd+Shift+H toggles call mode (works even when app is minimized)
listen('global-toggle-call', async () => {
  // Show window if hidden/minimized when starting call
  if (!S.callModeActive) {
    try {
      const { getCurrentWindow } = window.__TAURI__.window;
      const win = getCurrentWindow();
      await win.unminimize();
      await win.show();
      await win.setFocus();
    } catch (_) {}
  }
  toggleCallMode();
});

// Keyboard shortcut: Escape ends call
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && S.callModeActive) {
    e.preventDefault();
    endCallMode();
  }
});

// ── Exports ──

export {
  checkVoiceServer,
  startRecording,
  stopRecordingAndSend,
  cancelRecording,
  toggleCallMode,
  startCallMode,
  endCallMode,
  startWakeWordSSE,
  stopWakeWordSSE,
  handleCallTranscript,
  speakAndListen,
};
