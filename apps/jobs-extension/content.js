// content.js — floating button + mark-application panel.
// Shows the button on detected job pages; the toolbar popup can force-show
// the panel on any page ("hanni-show-panel" message).

const HANNI_STAGES = [
  ['found', 'Найдена'], ['saved', 'Сохранена'], ['applied', 'Отклик'],
  ['responded', 'Ответ'], ['interview', 'Интервью'], ['offer', 'Оффер'],
  ['accepted', 'Принято'], ['rejected', 'Отказ'], ['ignored', 'Пропущена'],
];

let hanniPanel = null;
let hanniFab = null;

function hanniApi(path, method, body) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage({ type: 'hanni-api', path, method, body }, resolve);
  });
}

function hanniEnsureFab() {
  if (hanniFab) return;
  hanniFab = document.createElement('button');
  hanniFab.className = 'hanni-ext-fab';
  hanniFab.title = 'Отметить вакансию в Hanni';
  hanniFab.textContent = 'H';
  hanniFab.addEventListener('click', hanniTogglePanel);
  document.documentElement.appendChild(hanniFab);
}

function hanniField(label, name, value, isArea) {
  const tag = isArea
    ? `<textarea class="hanni-ext-input" name="${name}" rows="2"></textarea>`
    : `<input class="hanni-ext-input" name="${name}">`;
  return `<label class="hanni-ext-row"><span>${label}</span>${tag}</label>`;
}

function hanniBuildPanel() {
  const stages = HANNI_STAGES
    .map(([v, l]) => `<option value="${v}"${v === 'applied' ? ' selected' : ''}>${l}</option>`)
    .join('');
  const panel = document.createElement('div');
  panel.className = 'hanni-ext-panel';
  panel.innerHTML = `
    <div class="hanni-ext-head">
      <span>Hanni — вакансия</span>
      <button class="hanni-ext-close" title="Закрыть">✕</button>
    </div>
    <div class="hanni-ext-status" hidden></div>
    ${hanniField('Позиция', 'position')}
    ${hanniField('Компания', 'company')}
    ${hanniField('Зарплата', 'salary')}
    <label class="hanni-ext-row"><span>Этап</span>
      <select class="hanni-ext-input" name="stage">${stages}</select></label>
    ${hanniField('Контакт', 'contact')}
    ${hanniField('Источник', 'source')}
    ${hanniField('Заметка', 'notes', '', true)}
    <button class="hanni-ext-save">Сохранить в Hanni</button>
  `;
  panel.querySelector('.hanni-ext-close').addEventListener('click', () => { panel.hidden = true; });
  panel.querySelector('.hanni-ext-save').addEventListener('click', hanniSave);
  document.documentElement.appendChild(panel);
  return panel;
}

function hanniSetStatus(panel, text, kind) {
  const st = panel.querySelector('.hanni-ext-status');
  st.hidden = !text;
  st.textContent = text || '';
  st.dataset.kind = kind || 'info';
}

function hanniFill(panel, data) {
  for (const name of ['position', 'company', 'salary', 'stage', 'contact', 'source', 'notes']) {
    const input = panel.querySelector(`[name="${name}"]`);
    if (input && data[name] != null && data[name] !== '') input.value = data[name];
  }
}

async function hanniTogglePanel() {
  if (hanniPanel && !hanniPanel.hidden) { hanniPanel.hidden = true; return; }
  if (!hanniPanel) hanniPanel = hanniBuildPanel();
  hanniPanel.hidden = false;

  const parsed = window.__hanniParseJob();
  hanniPanel.dataset.url = parsed.url;
  hanniFill(hanniPanel, parsed);
  hanniSetStatus(hanniPanel, 'Проверяю, есть ли в базе…', 'info');

  const res = await hanniApi(`/api/vacancy?url=${encodeURIComponent(parsed.url)}`, 'GET');
  if (!res || res.status === 0) {
    hanniSetStatus(hanniPanel, 'Hanni не отвечает — приложение запущено? (порт в настройках)', 'error');
  } else if (res.status === 401) {
    hanniSetStatus(hanniPanel, 'Неверный токен — вставь его в настройках расширения', 'error');
  } else if (res.ok && res.data && res.data.found) {
    hanniFill(hanniPanel, res.data.vacancy);
    const label = (HANNI_STAGES.find(([v]) => v === res.data.vacancy.stage) || [])[1] || res.data.vacancy.stage;
    hanniSetStatus(hanniPanel, `Уже в базе (этап: ${label}) — сохранение обновит запись`, 'ok');
  } else {
    hanniSetStatus(hanniPanel, 'Новой записью попадёт в таблицу Jobs', 'info');
  }
}

async function hanniSave() {
  const panel = hanniPanel;
  const get = (name) => panel.querySelector(`[name="${name}"]`).value.trim();
  const body = {
    url: panel.dataset.url || location.href.split('#')[0],
    position: get('position'), company: get('company'), salary: get('salary'),
    stage: get('stage'), contact: get('contact'), source: get('source'), notes: get('notes'),
  };
  if (!body.position && !body.company) {
    hanniSetStatus(panel, 'Заполни хотя бы позицию или компанию', 'error');
    return;
  }
  hanniSetStatus(panel, 'Сохраняю…', 'info');
  const res = await hanniApi('/api/vacancy', 'POST', body);
  if (res && res.ok) {
    hanniSetStatus(panel, res.data.created ? 'Сохранено в Jobs ✓' : 'Запись обновлена ✓', 'ok');
    if (hanniFab) hanniFab.classList.add('hanni-ext-fab-done');
  } else if (res && res.status === 401) {
    hanniSetStatus(panel, 'Неверный токен — настрой расширение', 'error');
  } else {
    hanniSetStatus(panel, `Ошибка: ${(res && (res.error || res.status)) || 'нет связи'}`, 'error');
  }
}

chrome.runtime.onMessage.addListener((msg) => {
  if (msg && msg.type === 'hanni-show-panel') {
    hanniEnsureFab();
    hanniTogglePanel();
  }
});

if (window.__hanniParseJob().detected) hanniEnsureFab();
