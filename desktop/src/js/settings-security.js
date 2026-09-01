// settings-security.js — Settings → Безопасность.
// Surfaces local API token controls and metadata-only diagnostics for fixed
// automation actions. Request bodies and executable scripts are never logged.

import { invoke } from './state.js';
import { escapeHtml, confirmModal } from './utils.js';

function formatTs(epochSecs) {
  if (!epochSecs) return '—';
  const d = new Date(epochSecs * 1000);
  const pad = (n) => String(n).padStart(2, '0');
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function renderLogRows(rows) {
  if (!rows.length) {
    return `<div class="settings-empty-hint">Журнал пуст — служебных действий ещё не было</div>`;
  }
  const head = `<thead><tr>
    <th>Время</th><th>Хэш действия</th><th>Статус</th><th>ms</th>
  </tr></thead>`;
  const body = rows.map(r => {
    const hash = (r.script_hash || '').slice(0, 8);
    const statusCls = r.success ? 'security-log-ok' : 'security-log-err';
    const statusTxt = r.success ? 'ok' : 'err';
    return `<tr>
      <td>${formatTs(r.ts)}</td>
      <td class="security-log-hash">${hash}</td>
      <td class="${statusCls}">${statusTxt}</td>
      <td>${r.duration_ms}</td>
    </tr>`;
  }).join('');
  return `<table class="security-log-table">${head}<tbody>${body}</tbody></table>`;
}

export async function renderSecuritySection() {
  let preview = '—';
  try { preview = await invoke('get_api_token_preview'); }
  catch (_) { /* missing token file is fine — show placeholder */ }
  let jobsPreview = '—';
  try { jobsPreview = await invoke('get_jobs_api_token_preview'); }
  catch (_) { /* missing token file is fine — show placeholder */ }

  let logRows = [];
  try { logRows = await invoke('list_automation_log', { limit: 100 }) || []; }
  catch (_) {}

  return `
    <div class="settings-section">
      <div class="settings-section-title">API Token</div>
      <div class="settings-row">
        <span class="settings-label">Текущий токен</span>
        <span class="settings-value">
          <code class="security-token-preview">${escapeHtml(preview)}</code>
        </span>
      </div>
      <div class="settings-row">
        <span class="settings-hint">Используется внешними клиентами для доступа к локальному API. На Windows хранится зашифрованным через DPAPI; полный токен выдаётся и копируется только при явном перевыпуске.</span>
      </div>
      <div class="settings-row" style="justify-content:flex-end;">
        <button class="btn-smallall" id="security-rotate-btn">Перевыпустить и скопировать</button>
      </div>
      <div class="settings-row">
        <span class="settings-label">Jobs-токен</span>
        <span class="settings-value">
          <code class="security-jobs-token-preview">${escapeHtml(jobsPreview)}</code>
        </span>
      </div>
      <div class="settings-row" style="justify-content:flex-end;">
        <button class="btn-smallall" id="security-rotate-jobs-btn">Перевыпустить и скопировать Jobs-токен</button>
      </div>
    </div>

    <div class="settings-section">
      <div class="settings-section-title">Журнал автоматизации</div>
      <div class="settings-row">
        <span class="settings-hint">Последние ${logRows.length} служебных действий. Хранится 7 дней. Записываются только время, SHA-256 идентификатора действия, результат и длительность; тела запросов и скрипты не сохраняются.</span>
      </div>
      <div id="security-log-wrap">${renderLogRows(logRows)}</div>
      <div class="settings-row" style="justify-content:flex-end;">
        <button class="btn-smallall" id="security-log-refresh">Обновить</button>
      </div>
    </div>
  `;
}

async function refreshLog(el) {
  const wrap = el.querySelector('#security-log-wrap');
  if (!wrap) return;
  try {
    const rows = await invoke('list_automation_log', { limit: 100 }) || [];
    wrap.innerHTML = renderLogRows(rows);
  } catch (_) {}
}

export function wireSecurityControls(el) {
  const wireRotate = (selector, command, previewCommand, previewSelector, label, buttonLabel) => {
    const rotateBtn = el.querySelector(selector);
    if (!rotateBtn) return;
    rotateBtn.addEventListener('click', async () => {
      const ok = await confirmModal(
        `Перевыпустить ${label}? Текущие внешние клиенты перестанут работать до перезапуска Hanni и обновления токена.`,
        'Перевыпустить'
      );
      if (!ok) return;
      rotateBtn.disabled = true;
      rotateBtn.textContent = 'Перевыпускаем…';
      try {
        const token = await invoke(command);
        await navigator.clipboard.writeText(token);
        const preview = el.querySelector(previewSelector);
        if (preview) {
          const newPreview = await invoke(previewCommand).catch(() => '—');
          preview.textContent = newPreview;
        }
        rotateBtn.textContent = 'Скопирован (нужен перезапуск Hanni)';
        setTimeout(() => {
          rotateBtn.textContent = buttonLabel;
          rotateBtn.disabled = false;
        }, 4000);
      } catch (_) {
        rotateBtn.textContent = 'Ошибка';
        setTimeout(() => {
          rotateBtn.textContent = buttonLabel;
          rotateBtn.disabled = false;
        }, 3000);
      }
    });
  };

  wireRotate(
    '#security-rotate-btn',
    'rotate_api_token',
    'get_api_token_preview',
    '.security-token-preview',
    'API-токен',
    'Перевыпустить и скопировать'
  );
  wireRotate(
    '#security-rotate-jobs-btn',
    'rotate_jobs_api_token',
    'get_jobs_api_token_preview',
    '.security-jobs-token-preview',
    'Jobs-токен',
    'Перевыпустить и скопировать Jobs-токен'
  );

  const refreshBtn = el.querySelector('#security-log-refresh');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', () => refreshLog(el));
  }
}
