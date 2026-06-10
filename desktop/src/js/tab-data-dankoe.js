// ── js/tab-data-dankoe.js — Dan Koe Protocol tab (split from tab-data.js) ──

import { S, invoke } from './state.js';
import { escapeHtml } from './utils.js';

// ── Dan Koe Protocol tab ──

const DK_PRACTICES = [
  {
    key: 'contemplation', label: 'Contemplation', icon: '🧘', hasText: true,
    what: 'Тихое созерцание — 10-15 минут без отвлечений',
    why: 'Тренирует осознанность, снижает реактивность ума, помогает замечать автоматические мысли вместо того, чтобы следовать за ними.',
    how: [
      'Сядь в тихое место, закрой глаза',
      'Не пытайся "очистить разум" — просто наблюдай мысли как облака',
      'Когда замечаешь, что увлёкся мыслью — мягко возвращайся к наблюдению',
      'Начни с 5 минут, постепенно увеличивай до 15-20',
    ],
    placeholder: 'Что заметил во время созерцания? Какие мысли возвращались? Что ощущал?',
  },
  {
    key: 'pattern_interrupt', label: 'Pattern Interrupt', icon: '⚡', hasText: false,
    what: 'Прерывание автоматических паттернов поведения',
    why: 'Большинство действий выполняются на автопилоте. Прерывая паттерны, ты возвращаешь контроль над вниманием и решениями.',
    how: [
      'Замечай моменты "на автопилоте": рука тянется к телефону, открываешь соцсети, ешь от скуки',
      'Спроси себя: "Зачем я это делаю прямо сейчас?"',
      'Сделай паузу на 10 секунд перед импульсивным действием',
      'Замени автоматическое действие осознанным выбором',
    ],
    examples: 'Потянулся к Instagram → паузу → "мне скучно, я могу почитать/погулять". Хочу сладкое → "голоден или стресс?"',
  },
  {
    key: 'vision', label: 'Vision', icon: '🔭', hasText: true,
    what: 'Вопросы о видении будущего — 5-10 минут рефлексии',
    why: 'Без ясного видения будущего ты реагируешь на обстоятельства вместо того, чтобы создавать жизнь по своему дизайну.',
    how: [
      'Задай себе один из вопросов ниже и запиши ответ',
      'Не фильтруй — пиши первое, что приходит',
      'Перечитывай свои ответы раз в неделю',
    ],
    questions: [
      'Какой будет моя идеальная жизнь через 3 года?',
      'Что бы я делал, если бы деньги не были проблемой?',
      'Какой навык изменит мою жизнь больше всего?',
      'От чего мне нужно отказаться, чтобы расти?',
      'Что я откладываю и почему?',
      'Каким человеком я хочу стать?',
    ],
    placeholder: 'Возьми один вопрос выше и пиши свободно. Не редактируй — фиксируй.',
  },
  {
    key: 'integration', label: 'Integration', icon: '🔗', hasText: true,
    what: 'Одно конкретное действие, основанное на практиках выше',
    why: 'Инсайты без действий — просто развлечение. Integration превращает осознанность в реальные изменения.',
    how: [
      'Выбери одну вещь из сегодняшних практик, которую можешь применить прямо сейчас',
      'Сделай это маленьким и конкретным: "напишу 200 слов", а не "начну писать книгу"',
      'Запиши, что сделал — это усиливает привычку',
    ],
    examples: 'Vision → "хочу быть здоровее" → Integration → "сегодня пойду на 30-мин прогулку вместо скролла"',
    placeholder: 'Какое одно конкретное действие сделаю сегодня?',
  },
];

const DK_TEXT_FIELD = { contemplation: 'contemplation_text', vision: 'vision_text', integration: 'integration_text' };
const DK_TEXT_TABS = ['contemplation', 'vision', 'integration'];
const DK_VIEWS = ['instructions', 'history'];

function dkWordCount(text) {
  return (text || '').trim().split(/\s+/).filter(Boolean).length;
}

function dkPreview(text, max = 80) {
  const t = (text || '').replace(/\s+/g, ' ').trim();
  return t.length > max ? t.slice(0, max) + '…' : t;
}

function dkRenderHeader(active) {
  const tabs = [
    { key: 'instructions', icon: '📖', label: 'Инструкция' },
    { key: 'history', icon: '📊', label: 'История' },
  ];
  const pills = tabs.map(t => {
    const isActive = active === t.key;
    return `<button class="dk-tab" data-view="${t.key}" style="padding:6px 14px;border-radius:var(--radius-1);border:1px solid var(--border-subtle);background:${isActive ? 'var(--bg-active)' : 'transparent'};color:${isActive ? 'var(--text-primary)' : 'var(--text-muted)'};font-size:13px;font-weight:${isActive ? '600' : '400'};cursor:pointer;">${t.icon} ${escapeHtml(t.label)}</button>`;
  }).join('');
  return `
    <div class="module-header" style="display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:16px;flex-wrap:wrap;">
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="font-size:22px;">🧠</span>
        <div>
          <h2 style="margin:0;font-size:18px;">Dan Koe Protocol</h2>
          <div style="font-size:12px;color:var(--text-muted);">Ежедневная практика осознанности · запись через календарь</div>
        </div>
      </div>
      <div style="display:flex;gap:6px;align-items:center;flex-wrap:wrap;">${pills}</div>
    </div>`;
}

function dkBindTabs(el) {
  el.querySelectorAll('.dk-tab').forEach(btn => {
    btn.addEventListener('click', () => {
      S._dkView = btn.dataset.view;
      loadDanKoe(S._dkView);
    });
  });
}

export async function loadDanKoe(subTab) {
  const el = document.getElementById('dankoe-content');
  if (!el) return;
  const view = DK_VIEWS.includes(subTab) ? subTab : (DK_VIEWS.includes(S._dkView) ? S._dkView : 'instructions');
  S._dkView = view;
  if (view === 'instructions') {
    dkRenderInstructions(el);
  } else {
    await dkRenderHistory(el);
  }
  dkBindTabs(el);
}

function dkRenderInstructions(el) {
  const cards = DK_PRACTICES.map(p => `
    <div style="padding:18px;border-radius:var(--radius-2);border:1px solid var(--border-subtle);background:var(--bg-card);margin-bottom:16px;">
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:10px;">
        <span style="font-size:22px;">${p.icon}</span>
        <span style="font-size:16px;font-weight:600;color:var(--text-primary);">${escapeHtml(p.label)}</span>
      </div>
      <div style="font-size:13px;color:var(--text-secondary);margin-bottom:10px;">${escapeHtml(p.what)}</div>
      <div style="font-size:12px;color:var(--text-muted);margin-bottom:12px;padding:8px 12px;background:var(--bg-hover);border-radius:var(--radius-1);"><b>Зачем:</b> ${escapeHtml(p.why)}</div>
      <div style="font-size:13px;color:var(--text-primary);font-weight:500;margin-bottom:6px;">Как делать:</div>
      <ul style="margin:0 0 10px 16px;padding:0;font-size:13px;color:var(--text-secondary);line-height:1.7;">
        ${p.how.map(h => `<li>${escapeHtml(h)}</li>`).join('')}
      </ul>
      ${p.questions ? `<div style="font-size:13px;color:var(--text-primary);font-weight:500;margin-bottom:6px;">Вопросы для рефлексии:</div>
        <ul style="margin:0 0 10px 16px;padding:0;font-size:13px;color:var(--text-muted);line-height:1.7;font-style:italic;">
          ${p.questions.map(q => `<li>${escapeHtml(q)}</li>`).join('')}
        </ul>` : ''}
      ${p.examples ? `<div style="font-size:12px;color:var(--text-faint);margin-top:6px;">💡 <i>${escapeHtml(p.examples)}</i></div>` : ''}
    </div>`).join('');
  el.innerHTML = `
    ${dkRenderHeader('instructions')}
    <div style="font-size:12px;color:var(--text-muted);margin-bottom:14px;padding:10px 12px;border-radius:var(--radius-1);background:var(--bg-hover);">
      ℹ️ Запись практик за день — через событие в календаре. Здесь — только описание и история.
    </div>
    ${cards}`;
}

async function dkRenderHistory(el) {
  const filter = DK_TEXT_TABS.includes(S._dkFilter) || S._dkFilter === 'all' ? S._dkFilter : 'all';
  const history = await invoke('get_dan_koe_history', { days: 30 }).catch(() => []);
  const meta = Object.fromEntries(DK_PRACTICES.map(p => [p.key, p]));
  const today = new Date().toISOString().slice(0, 10);

  const filterTypes = [
    { key: 'all', icon: '∗', label: 'All' },
    ...DK_TEXT_TABS.map(k => ({ key: k, icon: meta[k].icon, label: meta[k].label })),
  ];
  const chips = filterTypes.map(t => {
    const isActive = filter === t.key;
    return `<button class="dk-chip" data-filter="${t.key}" style="padding:6px 12px;border-radius:var(--radius-1);border:1px solid var(--border-subtle);background:${isActive ? 'var(--bg-active)' : 'transparent'};color:${isActive ? 'var(--text-primary)' : 'var(--text-muted)'};font-size:13px;font-weight:${isActive ? '600' : '400'};cursor:pointer;margin-right:6px;">${t.icon} ${escapeHtml(t.label)}</button>`;
  }).join('');

  const visibleKeys = filter === 'all' ? DK_TEXT_TABS : [filter];
  const flatRows = [];
  for (const e of history) {
    for (const k of visibleKeys) {
      const txt = e[DK_TEXT_FIELD[k]] || '';
      if (!txt.trim()) continue;
      flatRows.push({ date: e.date, key: k, text: txt });
    }
  }

  const showType = filter === 'all';
  const rows = flatRows.map((r, idx) => {
    const m = meta[r.key];
    const isToday = r.date === today;
    return `<tr class="dk-row" data-idx="${idx}" style="cursor:pointer;border-bottom:1px solid var(--border-subtle);">
      <td style="padding:8px 10px;font-size:13px;color:var(--text-primary);white-space:nowrap;">${escapeHtml(r.date)}${isToday ? ' <span style="color:var(--color-green);font-size:11px;">· сегодня</span>' : ''}</td>
      ${showType ? `<td style="padding:8px 10px;font-size:13px;color:var(--text-secondary);white-space:nowrap;">${m.icon} ${escapeHtml(m.label)}</td>` : ''}
      <td style="padding:8px 10px;font-size:13px;color:var(--text-secondary);">${escapeHtml(dkPreview(r.text))}</td>
      <td style="padding:8px 10px;font-size:12px;color:var(--text-muted);text-align:right;white-space:nowrap;">${dkWordCount(r.text)}</td>
    </tr>`;
  }).join('');

  const tableHeader = `
    <thead><tr style="background:var(--bg-hover);">
      <th style="padding:8px 10px;text-align:left;font-size:12px;color:var(--text-muted);font-weight:500;width:140px;">Дата</th>
      ${showType ? '<th style="padding:8px 10px;text-align:left;font-size:12px;color:var(--text-muted);font-weight:500;width:160px;">Тип</th>' : ''}
      <th style="padding:8px 10px;text-align:left;font-size:12px;color:var(--text-muted);font-weight:500;">Содержание</th>
      <th style="padding:8px 10px;text-align:right;font-size:12px;color:var(--text-muted);font-weight:500;width:60px;">Слов</th>
    </tr></thead>`;
  const colSpan = showType ? 4 : 3;
  const tableBody = rows
    ? `<tbody>${rows}</tbody>`
    : `<tbody><tr><td colspan="${colSpan}" style="padding:24px 10px;text-align:center;color:var(--text-muted);font-size:13px;">Пока пусто. Записи появятся здесь после заполнения через событие в календаре.</td></tr></tbody>`;
  const tableHtml = `<table style="width:100%;border-collapse:collapse;background:var(--bg-card);border-radius:var(--radius-2);overflow:hidden;border:1px solid var(--border-subtle);">${tableHeader}${tableBody}</table>`;

  el.innerHTML = `
    ${dkRenderHeader('history')}
    <div style="margin-bottom:14px;display:flex;flex-wrap:wrap;gap:6px;">${chips}</div>
    ${tableHtml}`;

  el.querySelectorAll('.dk-chip').forEach(btn => {
    btn.addEventListener('click', () => {
      S._dkFilter = btn.dataset.filter;
      dkRenderHistory(el).then(() => dkBindTabs(el));
    });
  });
  el.querySelectorAll('.dk-row').forEach(tr => {
    tr.addEventListener('click', () => {
      const r = flatRows[parseInt(tr.dataset.idx, 10)];
      if (!r) return;
      const m = meta[r.key];
      dkOpenViewer(r.date, m.label, m.icon, r.text);
    });
  });
}

function dkOpenViewer(date, label, icon, text) {
  document.querySelector('.dk-viewer-overlay')?.remove();
  const overlay = document.createElement('div');
  overlay.className = 'dk-viewer-overlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.4);z-index:10000;display:flex;align-items:center;justify-content:center;';
  overlay.innerHTML = `
    <div style="background:var(--bg-card);border-radius:var(--radius-2);max-width:600px;width:90%;max-height:80vh;overflow:auto;padding:20px;border:1px solid var(--border-subtle);">
      <div style="display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:12px;">
        <div style="font-size:14px;font-weight:600;color:var(--text-primary);">${icon} ${escapeHtml(label)} · ${escapeHtml(date)}</div>
        <button class="dk-close" style="background:none;border:none;font-size:18px;cursor:pointer;color:var(--text-muted);">×</button>
      </div>
      <div style="font-size:13px;color:var(--text-primary);white-space:pre-wrap;line-height:1.6;">${escapeHtml(text)}</div>
    </div>`;
  document.body.appendChild(overlay);
  overlay.addEventListener('click', e => { if (e.target === overlay) overlay.remove(); });
  overlay.querySelector('.dk-close')?.addEventListener('click', () => overlay.remove());
}
