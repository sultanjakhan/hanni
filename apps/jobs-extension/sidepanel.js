// sidepanel.js — persistent side panel: shows the vacancy parsed from the
// active tab, saves into Hanni, and hosts the settings (port + token).
// Stays open across tabs/navigation — refreshes itself on tab changes.

const $ = (id) => document.getElementById(id);
const portButtons = [...document.querySelectorAll('.seg button')];
const STAGE_LABELS = {
  found: 'Найдена', saved: 'Сохранена', applied: 'Отклик', responded: 'Ответ',
  interview: 'Интервью', offer: 'Оффер', accepted: 'Принято', rejected: 'Отказ', ignored: 'Пропущена',
};
let currentUrl = '';

function setStatus(text, kind) {
  $('status').textContent = text;
  $('status').className = kind || '';
}

const api = (path, method, body) =>
  chrome.runtime.sendMessage({ type: 'hanni-api', path, method, body });

// ── vacancy form ──────────────────────────────────────────────────────────

async function refresh() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !/^https?:\/\//.test(tab.url || '')) {
    setStatus('Открой страницу вакансии в активной вкладке');
    return;
  }
  let parsed = null;
  try { parsed = await chrome.tabs.sendMessage(tab.id, { type: 'hanni-get-parse' }); } catch { /* no content script in this tab yet */ }
  if (!parsed) {
    setStatus('Не вижу содержимое вкладки — обнови страницу (F5)', 'error');
    return;
  }
  currentUrl = parsed.url;
  $('position').value = parsed.position || '';
  $('company').value = parsed.company || '';
  $('salary').value = parsed.salary || '';
  $('source').value = parsed.source || '';
  $('contact').value = '';
  $('notes').value = '';
  $('stage').value = 'applied';

  setStatus('Проверяю, есть ли в базе…');
  const res = await api(`/api/vacancy?url=${encodeURIComponent(parsed.url)}`, 'GET');
  if (!res || res.status === 0) {
    setStatus('Hanni не отвечает — приложение запущено?', 'error');
  } else if (res.status === 401) {
    setStatus('Неверный токен — вставь его в настройках ниже', 'error');
    $('settings').open = true;
  } else if (res.ok && res.data && res.data.found) {
    const v = res.data.vacancy;
    for (const k of ['position', 'company', 'salary', 'stage', 'contact', 'source', 'notes']) {
      if (v[k] != null && v[k] !== '') $(k).value = v[k];
    }
    setStatus(`Уже в базе (этап: ${STAGE_LABELS[v.stage] || v.stage}) — сохранение обновит запись`, 'ok');
  } else {
    setStatus('Новой записью попадёт в таблицу Jobs');
  }
}

async function save() {
  const body = {
    url: currentUrl || '',
    position: $('position').value.trim(),
    company: $('company').value.trim(),
    salary: $('salary').value.trim(),
    stage: $('stage').value,
    contact: $('contact').value.trim(),
    source: $('source').value.trim(),
    notes: $('notes').value.trim(),
  };
  if (!body.url) { setStatus('Нет адреса вакансии — открой её страницу', 'error'); return; }
  if (!body.position && !body.company) {
    setStatus('Заполни хотя бы позицию или компанию', 'error');
    return;
  }
  setStatus('Сохраняю…');
  const res = await api('/api/vacancy', 'POST', body);
  if (res && res.ok) setStatus(res.data && res.data.created ? 'Сохранено в Jobs ✓' : 'Запись обновлена ✓', 'ok');
  else if (res && res.status === 401) { setStatus('Неверный токен — вставь его в настройках ниже', 'error'); $('settings').open = true; }
  else setStatus(`Ошибка: ${(res && (res.error || res.status)) || 'нет связи'}`, 'error');
}

// ── settings ──────────────────────────────────────────────────────────────

function selectedPort() {
  const active = portButtons.find((b) => b.classList.contains('active'));
  return active ? parseInt(active.dataset.port, 10) : 8235;
}

function renderPort(port) {
  for (const b of portButtons) b.classList.toggle('active', b.dataset.port === String(port));
}

// token.local.js (generated from api_token.txt) is the source of truth when
// present; manual paste is only the fallback. Hanni tokens are UUIDs, so on
// manual input we extract the UUID from whatever was pasted around it.
const UUID_RE = /[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/;
const fileToken = (self.HANNI_LOCAL_TOKEN || '').trim();

function renderTokenState(token) {
  const el = $('token-state');
  if (fileToken) {
    el.textContent = `Токен из token.local.js ✓ (…${fileToken.slice(-4)})`;
    el.className = 'ok';
  } else if (!token) {
    el.textContent = 'Токен не задан';
    el.className = '';
  } else if (!UUID_RE.test(token)) {
    el.textContent = `Сохранено, но не похоже на токен (${token.length} символов вместо 36)`;
    el.className = 'error';
  } else {
    el.textContent = `Токен сохранён ✓ (…${token.slice(-4)})`;
    el.className = 'ok';
  }
}

async function saveSettings() {
  let token = fileToken;
  if (!token) {
    const raw = $('token').value;
    const m = raw.match(UUID_RE);
    token = m ? m[0] : raw.trim();
    if (m && raw.trim() !== m[0]) $('token').value = m[0];
  }
  await chrome.storage.sync.set({ port: selectedPort(), token });
  renderTokenState(token);
}

async function checkConnection() {
  await saveSettings();
  setStatus('Проверяю…');
  const res = await api('/api/vacancy?url=__ping__', 'GET');
  if (!res || res.status === 0) setStatus('Hanni не отвечает — приложение запущено?', 'error');
  else if (res.status === 401) setStatus('Сервер доступен, но токен неверный', 'error');
  else if (res.status === 404) setStatus('Hanni без роута вакансий — обнови приложение', 'error');
  else if (res.ok) setStatus('Связь и токен в порядке ✓', 'ok');
  else setStatus(`Неожиданный ответ: ${res.status}`, 'error');
}

// ── wiring ────────────────────────────────────────────────────────────────

for (const b of portButtons) {
  b.addEventListener('click', () => { renderPort(b.dataset.port); checkConnection(); });
}
$('token').addEventListener('input', saveSettings);
$('token').addEventListener('change', checkConnection);
$('check').addEventListener('click', checkConnection);
$('save').addEventListener('click', save);
$('refresh').addEventListener('click', refresh);

chrome.tabs.onActivated.addListener(refresh);
chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => {
  if (tab.active && changeInfo.status === 'complete') refresh();
});

(async function init() {
  const { port = 8235, token = '' } = await chrome.storage.sync.get(['port', 'token']);
  renderPort(port);
  if (fileToken) {
    $('token').value = fileToken;
    $('token').disabled = true;
    await chrome.storage.sync.set({ token: fileToken });
    renderTokenState(fileToken);
    refresh();
    return;
  }
  $('token').value = token;
  renderTokenState(token);
  if (!token) {
    $('settings').open = true;
    setStatus('Вставь токен из api_token.txt в настройках ниже', 'error');
    return;
  }
  refresh();
})();
