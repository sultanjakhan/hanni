// ── js/actions.js — Action parsing & execution (LLM action blocks) ──

import { S, invoke, tabLoaders } from './state.js';

// ── repairJson ──

function repairJson(str) {
  // Fix common model JSON mistakes
  let s = str.trim();
  // Remove trailing commas before } or ]
  s = s.replace(/,\s*([}\]])/g, '$1');
  // Replace single quotes with double quotes (but not inside strings)
  if (!s.includes('"') && s.includes("'")) {
    s = s.replace(/'/g, '"');
  }
  // Fix unquoted keys: {action: "foo"} → {"action": "foo"}
  s = s.replace(/([{,])\s*([a-zA-Z_]\w*)\s*:/g, '$1"$2":');
  return s;
}

// ── parseAndExecuteActions ──

export function parseAndExecuteActions(text) {
  // Lenient regex: optional newlines, handle ```action\n...\n``` and ```action{...}```
  const actionRegex = /```action\s*\n?([\s\S]*?)```/g;
  let match;
  const actions = [];

  while ((match = actionRegex.exec(text)) !== null) {
    let json = match[1].trim();
    if (json) actions.push(repairJson(json));
  }

  // Fallback: if no ```action found, try to find bare JSON with "action" key
  if (actions.length === 0) {
    const bareJson = text.match(/\{[^{}]*"action"\s*:\s*"[^"]+?"[^{}]*\}/g);
    if (bareJson) {
      for (const j of bareJson) actions.push(repairJson(j));
    }
  }

  return actions;
}

// ── executeAction ──

export async function executeAction(actionJson) {
  try {
    const action = JSON.parse(actionJson);
    // Normalize common model variations
    if (action.meal_type) {
      const mealMap = {'завтрак':'breakfast','обед':'lunch','ужин':'dinner','перекус':'snack',
        'breakfast':'breakfast','lunch':'lunch','dinner':'dinner','snack':'snack'};
      action.meal_type = mealMap[action.meal_type.toLowerCase()] || action.meal_type.toLowerCase();
    }
    let actionType = action.action || action.type;
    let result;

    // S5: Confirmation for dangerous actions
    const DANGEROUS_ACTIONS = ['run_shell', 'close_app', 'quit_app', 'open_app', 'start_focus'];
    if (DANGEROUS_ACTIONS.includes(actionType)) {
      const desc = actionType === 'run_shell' ? `Команда: ${action.command || action.cmd || '?'}`
        : actionType === 'start_focus' ? `Фокус: ${action.duration || '?'} мин`
        : actionType === 'open_app' ? `Открыть: ${action.name || action.app || '?'}`
        : `Закрыть: ${action.name || action.app || '?'}`;
      const confirmed = await new Promise(resolve => {
        const overlay = document.createElement('div');
        overlay.className = 'confirm-overlay';
        const modal = document.createElement('div');
        modal.className = 'confirm-modal';
        const title = document.createElement('div');
        title.className = 'confirm-title';
        title.textContent = 'Подтверждение действия';
        const descEl = document.createElement('div');
        descEl.className = 'confirm-desc';
        descEl.textContent = desc;
        const btns = document.createElement('div');
        btns.className = 'confirm-buttons';
        btns.innerHTML = '<button class="confirm-cancel">Отмена</button><button class="confirm-ok">Выполнить</button>';
        modal.append(title, descEl, btns);
        overlay.appendChild(modal);
        document.body.appendChild(overlay);
        overlay.querySelector('.confirm-cancel').onclick = () => { overlay.remove(); resolve(false); };
        overlay.querySelector('.confirm-ok').onclick = () => { overlay.remove(); resolve(true); };
      });
      if (!confirmed) return { success: false, result: 'Действие отменено пользователем' };
    }

    switch (actionType) {
      case 'add_purchase':
        result = await invoke('add_transaction', {
          transactionType: 'expense',
          amount: action.amount,
          category: action.category || 'other',
          description: action.description || '',
          currency: 'KZT'
        });
        break;
      case 'add_time':
        result = await invoke('tracker_add_time', {
          activity: action.activity || '',
          duration: action.duration || 0,
          category: action.category || 'other',
          productive: action.productive !== false
        });
        break;
      case 'add_goal':
        result = await invoke('tracker_add_goal', {
          title: action.title || '',
          category: action.category || 'other'
        });
        break;
      case 'add_note':
      case 'create_note':
        result = await invoke('create_note', {
          title: action.title || '',
          content: action.content || '',
          tags: action.tags || '',
          tabName: action.tab || null,
          status: action.status || 'note',
          dueDate: action.due_date || null,
          reminderAt: action.remind_at || null,
        });
        if (S.callModeActive && action.save_audio) {
          try {
            const wavPath = await invoke('save_voice_note', { title: action.title || 'note' });
            result = (result || '') + ' (audio: ' + wavPath + ')';
          } catch (_) {}
        }
        break;
      case 'search_notes': {
        const filter = action.tab ? `tab:${action.tab}` : null;
        const notes = await invoke('get_notes', { filter, search: action.query || null });
        result = JSON.stringify((notes || []).slice(0, 5).map(n => ({ id: n.id, title: n.title, tags: n.tags, status: n.status })));
        break;
      }
      case 'complete_task':
        await invoke('update_note_status', { id: action.id, status: 'done' });
        result = 'Задача отмечена как выполненная';
        break;
      case 'get_stats':
        result = await invoke('tracker_get_stats');
        break;
      case 'get_activity':
        result = await invoke('get_activity_summary');
        break;
      case 'get_calendar':
        result = await invoke('get_calendar_events');
        break;
      case 'get_music':
        result = await invoke('get_now_playing');
        break;
      case 'get_browser':
        result = await invoke('get_browser_tab');
        break;
      case 'remember':
        result = await invoke('memory_remember', {
          category: action.category || 'user',
          key: action.key || '',
          value: action.value || ''
        });
        // ME2: Show memory notification
        { const tag = document.createElement('div');
          tag.className = 'memory-toast';
          tag.textContent = `Запомнила: ${action.key || action.value || ''}`;
          document.getElementById('chat')?.appendChild(tag);
          setTimeout(() => tag.classList.add('fade-out'), 3000);
          setTimeout(() => tag.remove(), 3500); }
        break;
      case 'recall':
        result = await invoke('memory_recall', {
          category: action.category || 'user',
          key: action.key || null
        });
        break;
      case 'forget':
        result = await invoke('memory_forget', {
          category: action.category || 'user',
          key: action.key || ''
        });
        break;
      case 'search_memory':
        result = await invoke('memory_search', {
          query: action.query || '',
          limit: action.limit || null
        });
        break;
      // Focus mode actions
      case 'start_focus':
        result = await invoke('start_focus', {
          durationMinutes: action.duration || 30,
          apps: action.apps || null,
          sites: action.sites || null,
        });
        break;
      case 'stop_focus':
        result = await invoke('stop_focus');
        break;
      // System actions
      case 'run_shell':
        result = await invoke('run_shell', { command: action.command || '' });
        break;
      case 'open_url':
        result = await invoke('open_url', { url: action.url || '' });
        break;
      case 'send_notification':
        result = await invoke('send_notification', {
          title: action.title || 'Hanni',
          body: action.body || ''
        });
        break;
      case 'set_volume':
        result = await invoke('set_volume', { level: action.level || 50 });
        break;
      case 'get_clipboard':
        result = await invoke('get_clipboard');
        break;
      case 'set_clipboard':
        result = await invoke('set_clipboard', { text: action.text || '' });
        break;
      case 'web_search':
      case 'search_web':
        result = await invoke('web_search', { query: action.query || '' });
        break;
      case 'read_url':
      case 'fetch_url':
        result = await invoke('read_url', { url: action.url || '' });
        break;
      // Media
      case 'add_media_item':
      case 'add_media':
        result = await invoke('add_media_item', {
          mediaType: action.media_type || 'movie', title: action.title || '',
          originalTitle: action.original_title || null, year: action.year || null,
          description: action.description || null, coverUrl: action.cover_url || null,
          status: action.status || 'planned', rating: action.rating || null,
          progress: action.progress || null, totalEpisodes: action.total_episodes || null,
          notes: action.notes || null,
        });
        break;
      // Food
      case 'log_food':
        result = await invoke('log_food', {
          date: action.date || null, mealType: action.meal_type || 'snack',
          name: action.name || '', calories: action.calories || null,
          protein: action.protein || null, carbs: action.carbs || null,
          fat: action.fat || null, notes: action.notes || null,
        });
        break;
      case 'add_product':
        result = await invoke('add_product', {
          name: action.name || '', category: action.category || null,
          quantity: action.quantity || null, unit: action.unit || null,
          expiryDate: action.expiry_date || null, location: action.location || 'fridge',
          barcode: action.barcode || null, notes: action.notes || null,
        });
        break;
      // Money
      case 'add_transaction':
        result = await invoke('add_transaction', {
          date: action.date || null, transactionType: action.transaction_type || action.tx_type || 'expense',
          amount: action.amount || 0, currency: action.currency || 'KZT',
          category: action.category || 'other', description: action.description || '',
          recurring: action.recurring || false, recurringPeriod: action.recurring_period || null,
        });
        break;
      // Calendar events
      case 'create_event':
        result = await invoke('create_event', {
          title: action.title || '', description: action.description || '',
          date: action.date || new Date().toISOString().slice(0, 10),
          time: action.time || '', durationMinutes: action.duration || action.duration_minutes || 60,
          category: action.category || 'general', color: action.color || '#9B9B9B',
        });
        break;
      case 'delete_event':
        result = await invoke('delete_event', { id: action.id });
        break;
      case 'sync_calendar':
        try {
          const m = action.month || (new Date().getMonth() + 1);
          const y = action.year || new Date().getFullYear();
          result = await invoke('sync_apple_calendar', { month: m, year: y });
        } catch (e) { result = 'Sync error: ' + e; }
        break;
      // Activities (time tracking)
      case 'start_activity':
        result = await invoke('start_activity', {
          title: action.name || action.title || action.activity || '',
          category: action.category || 'other',
          focusMode: action.focus_mode || false,
          duration: action.duration || null,
          apps: null, sites: null,
        });
        setTimeout(() => tabLoaders.updateFocusWidget?.(), 100);
        break;
      case 'stop_activity':
        result = await invoke('stop_activity');
        setTimeout(() => tabLoaders.updateFocusWidget?.(), 100);
        break;
      case 'get_current_activity':
        result = await invoke('get_current_activity');
        break;
      case 'start_pomodoro': {
        const title = action.title || action.name || 'Помодоро';
        const cat = action.category || 'work';
        tabLoaders.startPomodoro?.(title, cat, action.focus_mode || false);
        result = `Помодоро запущен: ${title} (25 мин)`;
        setTimeout(() => tabLoaders.updateFocusWidget?.(), 100);
        break;
      }
      case 'stop_pomodoro':
        S.pomodoroState.active = false;
        await invoke('stop_activity').catch(() => {});
        result = 'Помодоро остановлен';
        setTimeout(() => tabLoaders.updateFocusWidget?.(), 100);
        break;
      // Navigation
      case 'open_tab':
        tabLoaders.switchTab?.(action.tab || 'chat');
        result = `Открыта вкладка: ${action.tab}`;
        break;
      case 'open_note': {
        const noteId = action.id || action.note_id;
        if (noteId) {
          tabLoaders.switchTab?.('notes');
          setTimeout(() => {
            S.currentNoteId = noteId;
            S.notesViewMode = 'edit';
            const notesEl = document.getElementById('notes-content');
            if (notesEl) tabLoaders.renderNoteEditor?.(notesEl, noteId);
          }, 100);
          result = `Заметка #${noteId} открыта`;
        } else { result = 'Не указан id заметки'; }
        break;
      }
      case 'get_tasks': {
        const taskFilter = action.status || 'tasks';
        const tasks = await invoke('get_notes', { filter: taskFilter, search: action.query || null });
        result = JSON.stringify((tasks || []).slice(0, 10).map(n => ({
          id: n.id, title: n.title, status: n.status, due_date: n.due_date, tags: n.tags
        })));
        break;
      }
      case 'create_task': {
        const taskId = await invoke('create_note', {
          title: action.title || '', content: action.content || '',
          tags: action.tags || '', status: 'task',
          tabName: action.tab || null,
          dueDate: action.due_date || null,
          reminderAt: action.remind_at || null,
        });
        result = `Задача создана (id: ${taskId})`;
        { const tag = document.createElement('div');
          tag.className = 'memory-toast';
          tag.textContent = `\u2705 Задача: ${action.title || 'Новая задача'}${action.due_date ? ' \u2192 ' + action.due_date : ''}`;
          tag.style.cursor = 'pointer';
          tag.addEventListener('click', () => { tabLoaders.switchTab?.('notes'); setTimeout(() => { S.notesFilters = new Set(['task']); tabLoaders.loadNotes?.(); }, 100); });
          document.getElementById('chat')?.appendChild(tag);
          setTimeout(() => tag.classList.add('fade-out'), 4000);
          setTimeout(() => tag.remove(), 4500); }
        break;
      }
      // Projects & Tasks (legacy — work tab)
      case 'create_project_task':
        result = await invoke('create_task', {
          projectId: action.project_id || 1, title: action.title || '',
          description: action.description || '', priority: action.priority || 'medium',
          dueDate: action.due_date || null,
        });
        break;
      // Home items
      case 'add_home_item':
        result = await invoke('add_home_item', {
          name: action.name || '', category: action.category || 'other',
          quantity: action.quantity || null, unit: action.unit || null,
          location: action.location || 'other', notes: action.notes || null,
        });
        break;
      // Health
      case 'log_health': {
        // Rust log_health takes one type at a time — call for each provided field
        const fields = {sleep: action.sleep, water: action.water, steps: action.steps, weight: action.weight};
        const logged = [];
        for (const [type, val] of Object.entries(fields)) {
          if (val != null && val !== undefined) {
            await invoke('log_health', { healthType: type, value: Number(val), notes: action.notes || null });
            logged.push(`${type}=${val}`);
          }
        }
        result = logged.length ? `Записано: ${logged.join(', ')}` : 'Нет данных для записи';
        break;
      }
      case 'add_workout':
      case 'create_workout':
      case 'log_workout':
        result = await invoke('create_workout', {
          workoutType: action.type || action.workout_type || 'general',
          title: action.title || action.name || 'Тренировка',
          durationMinutes: action.duration || action.duration_minutes || 60,
          calories: action.calories || null,
          notes: action.notes || '',
        });
        break;
      // Goals
      case 'create_goal':
        result = await invoke('create_goal', {
          tabName: action.tab || 'general', title: action.title || '',
          targetValue: action.target || 1, currentValue: action.current || 0,
          unit: action.unit || '', deadline: action.deadline || null,
        });
        break;
      case 'update_goal':
        result = await invoke('update_goal', {
          id: action.id, currentValue: action.current || null,
          status: action.status || null,
        });
        break;
      // Reminders
      case 'set_reminder':
      case 'remind':
      case 'set_timer':
        result = await invoke('set_reminder', {
          title: action.title || action.text || '',
          remindAt: action.remind_at || action.time || '',
          repeat: action.repeat || null,
        });
        break;
      // App & Music control
      case 'open_app':
      case 'launch_app':
        result = await invoke('open_app', { name: action.name || action.app || '' });
        break;
      case 'close_app':
      case 'quit_app':
        result = await invoke('close_app', { name: action.name || action.app || '' });
        break;
      case 'music':
      case 'music_control':
        result = await invoke('music_control', { action: action.command || action.action_type || 'toggle' });
        break;
      default:
        // Try MCP servers for unknown actions
        try {
          result = await invoke('mcp_call_tool', { name: actionType, arguments: action });
        } catch (mcpErr) {
          console.warn('Unknown action (no MCP match):', actionType, action);
          result = 'Unknown action: ' + actionType;
        }
    }

    return { success: true, result };
  } catch (e) {
    return { success: false, result: String(e) };
  }
}
