// popup.js — extension settings (port + token) and connection check.

const $ = (id) => document.getElementById(id);

function setStatus(text, kind) {
  $('status').textContent = text;
  $('status').className = kind || '';
}

async function loadSettings() {
  const { port = 8235, token = '' } = await chrome.storage.sync.get(['port', 'token']);
  $('port').value = String(port);
  $('token').value = token;
}

async function saveSettings() {
  await chrome.storage.sync.set({
    port: parseInt($('port').value, 10),
    token: $('token').value.trim(),
  });
  setStatus('Настройки сохранены', 'ok');
}

// /api/status needs no auth (server up?), then an authed call validates the token.
async function checkConnection() {
  await saveSettings();
  const port = $('port').value;
  const token = $('token').value.trim();
  setStatus('Проверяю…');
  try {
    const res = await fetch(`http://127.0.0.1:${port}/api/status`);
    if (!res.ok) { setStatus(`Сервер ответил ${res.status}`, 'error'); return; }
  } catch {
    setStatus('Hanni не отвечает — приложение запущено?', 'error');
    return;
  }
  try {
    const res = await fetch(`http://127.0.0.1:${port}/api/vacancy?url=__ping__`, {
      headers: { 'Authorization': `Bearer ${token}` },
    });
    if (res.status === 401) setStatus('Сервер доступен, но токен неверный', 'error');
    else if (res.ok) setStatus('Связь и токен в порядке ✓', 'ok');
    else setStatus(`Неожиданный ответ: ${res.status}`, 'error');
  } catch (e) {
    setStatus(`Ошибка: ${e}`, 'error');
  }
}

async function markCurrentPage() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !tab.id) return;
  try {
    await chrome.tabs.sendMessage(tab.id, { type: 'hanni-show-panel' });
    window.close();
  } catch {
    setStatus('Не получилось — обнови страницу и попробуй снова', 'error');
  }
}

$('save').addEventListener('click', saveSettings);
$('check').addEventListener('click', checkConnection);
$('mark').addEventListener('click', markCurrentPage);
loadSettings();
