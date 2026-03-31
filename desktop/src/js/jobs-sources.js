// jobs-sources.js — Job sources with 2 sub-tabs: Sites & Telegram
import { invoke, S } from './state.js';
import { escapeHtml } from './utils.js';
import { showEditModal, EDIT_ICON } from './jobs-edit.js';

export async function renderSources(el) {
  let entries = [];
  try { entries = await invoke('memory_list', { category: 'jobs_sources', limit: 100 }); } catch {}

  if (entries.length === 0) {
    el.innerHTML = `<div class="jm-empty">
      <div class="jm-empty-icon">🔍</div>
      <div class="jm-empty-title">Источники не заданы</div>
      <div class="jm-empty-desc">Сайты, Telegram-каналы и другие источники вакансий</div>
      <button class="btn btn-sm btn-primary jm-seed-src-btn">Заполнить</button>
    </div>`;
    el.querySelector('.jm-seed-src-btn')?.addEventListener('click', async () => {
      await seedSources();
      await renderSources(el);
    });
    return;
  }

  if (!S._srcTab) S._srcTab = 'site';
  const active = S._srcTab;

  const sites = [], tg = [];
  entries.forEach(e => {
    const d = JSON.parse(e.value || '{}');
    d._key = e.key;
    if (d.type === 'telegram') tg.push(d); else sites.push(d);
  });

  const tabsHtml = `<div class="jm-tabs-bar jm-tabs-sub">
    <div class="jm-tab${active === 'site' ? ' active' : ''}" data-src-tab="site">Сайты (${sites.length})</div>
    <div class="jm-tab${active === 'telegram' ? ' active' : ''}" data-src-tab="telegram">TG-каналы (${tg.length})</div>
  </div>`;

  const items = active === 'site' ? sites : tg;
  const cards = items.map(d => sourceCard(d)).join('');
  const addType = active === 'site' ? 'site' : 'telegram';
  const addLabel = active === 'site' ? '+ Добавить сайт' : '+ Добавить канал';

  el.innerHTML = `${tabsHtml}<div class="jm-resume">${cards}
    <div style="margin-top:var(--space-2)"><button class="btn btn-sm jm-add-src-btn">${addLabel}</button></div>
  </div>`;

  el.querySelectorAll('[data-src-tab]').forEach(tab => {
    tab.addEventListener('click', () => { S._srcTab = tab.dataset.srcTab; renderSources(el); });
  });
  wireSourceEvents(el, entries, addType);
}

function sourceCard(d) {
  const url = d.url ? `<a href="${escapeHtml(d.url)}" class="jm-card-link" target="_blank">${escapeHtml(d.url)}</a>` : '';
  const time = d.schedule || '—';
  return `<div class="jm-card"><div class="jm-card-top">
    <span class="jm-card-title">${escapeHtml(d.name || d._key)}</span>
    <span class="jm-card-schedule" title="Время обхода">${escapeHtml(time)}</span>
    <button class="jm-edit-btn" data-src-key="${escapeHtml(d._key)}">${EDIT_ICON}</button>
  </div>
  ${d.description ? `<div class="jm-card-sub">${escapeHtml(d.description)}</div>` : ''}
  ${url}</div>`;
}

function wireSourceEvents(el, entries, addType) {
  el.querySelectorAll('[data-src-key]').forEach(btn => {
    btn.addEventListener('click', () => {
      const entry = entries.find(e => e.key === btn.dataset.srcKey);
      const d = entry ? JSON.parse(entry.value || '{}') : {};
      showEditModal('jobs_sources', btn.dataset.srcKey, {
        name: { label: 'Название', value: d.name },
        type: { label: 'Тип (site/telegram)', value: d.type },
        url: { label: 'URL / ссылка', value: d.url },
        description: { label: 'Описание', value: d.description },
        schedule: { label: 'Время обхода (HH:MM)', value: d.schedule || '' },
      }, () => renderSources(el));
    });
  });
  el.querySelector('.jm-add-src-btn')?.addEventListener('click', () => {
    const nextIdx = entries.length + 1;
    showEditModal('jobs_sources', `source_${nextIdx}`, {
      name: { label: 'Название', value: '' },
      type: { label: 'Тип', value: addType },
      url: { label: 'URL / ссылка', value: '' },
      description: { label: 'Описание', value: '' },
      schedule: { label: 'Время обхода (HH:MM)', value: '' },
    }, () => renderSources(el));
  });
}

export async function seedSources() {
  const sources = [
    // Sites — staggered from 04:00
    { key: 'hh', value: { name: 'hh.kz', type: 'site', url: 'https://hh.kz', description: 'Основной джоб-борд Казахстана (+ hh.ru)', schedule: '04:00' } },
    { key: 'linkedin', value: { name: 'LinkedIn Jobs', type: 'site', url: 'https://linkedin.com/jobs', description: 'Через агрегатор (аккаунт заблокирован)', schedule: '04:20' } },
    { key: 'vseti', value: { name: 'VSETI', type: 'site', url: 'https://vseti.app', description: 'IT-агрегатор вакансий', schedule: '04:40' } },
    { key: 'geekjob', value: { name: 'GeekJob', type: 'site', url: 'https://geekjob.ru', description: 'IT-вакансии, PM/аналитика', schedule: '05:00' } },
    { key: 'habr', value: { name: 'Habr Career', type: 'site', url: 'https://career.habr.com', description: 'IT/продукт вакансии', schedule: '05:20' } },
    { key: 'djinni', value: { name: 'Djinni', type: 'site', url: 'https://djinni.co', description: 'CIS, PM/BA, фриланс + full-time', schedule: '05:40' } },
    { key: 'x', value: { name: 'X (Twitter)', type: 'site', url: 'https://x.com', description: 'PM-комьюнити, вакансии в тредах', schedule: '06:00' } },
    // Telegram — staggered from 07:00 (10 min apart)
    { key: 'tg_forproducts', value: { name: 'Job for Products & Projects', type: 'telegram', url: 'https://t.me/forproducts', description: 'PM/PO/CPO вакансии (~20K)', schedule: '07:00' } },
    { key: 'tg_hireproproduct', value: { name: 'Hire ProProduct', type: 'telegram', url: 'https://t.me/hireproproduct', description: 'Продуктовые вакансии РФ и зарубеж (~13K)', schedule: '07:10' } },
    { key: 'tg_pmclub', value: { name: 'PMCLUB Jobs', type: 'telegram', url: 'https://t.me/pmclub', description: 'Еженедельные подборки PM', schedule: '07:20' } },
    { key: 'tg_products_jobs', value: { name: 'Products Jobs', type: 'telegram', url: 'https://t.me/products_jobs', description: 'Только продуктовые вакансии (~10K)', schedule: '07:30' } },
    { key: 'tg_productjobgo', value: { name: 'Вакансии продакт-менеджеров', type: 'telegram', url: 'https://t.me/productjobgo', description: 'Кураторский канал от Fresh PM', schedule: '07:40' } },
    { key: 'tg_blackproduct', value: { name: 'Black Product Owner', type: 'telegram', url: 'https://t.me/blackproduct', description: 'Авторский канал, кураторский отбор', schedule: '07:50' } },
    { key: 'tg_projects_jobs', value: { name: 'Projects Jobs', type: 'telegram', url: 'https://t.me/projects_jobs', description: 'PM-вакансии с зарплатой (~6K)', schedule: '08:00' } },
    { key: 'tg_agile_jobs', value: { name: 'Agile Jobs', type: 'telegram', url: 'https://t.me/agile_jobs', description: 'PM, Scrum Master, Agile Coach (~5K)', schedule: '08:10' } },
    { key: 'tg_workitkz', value: { name: 'IT Вакансии Казахстан', type: 'telegram', url: 'https://t.me/workitkz', description: 'IT КЗ с зарплатами', schedule: '08:20' } },
    { key: 'tg_jobkz', value: { name: 'JobKZ', type: 'telegram', url: 'https://t.me/jobkz_1', description: 'Вакансии по всему Казахстану (~17K)', schedule: '08:30' } },
    { key: 'tg_vacancykz', value: { name: 'КЗ на IT удалёнке', type: 'telegram', url: 'https://t.me/vacancykz', description: '100% удалённые вакансии для КЗ', schedule: '08:40' } },
    { key: 'tg_foranalysts', value: { name: 'Job for Analysts', type: 'telegram', url: 'https://t.me/foranalysts', description: 'Аналитики и Data Scientists', schedule: '08:50' } },
    { key: 'tg_analyst_job', value: { name: 'Работа для аналитиков', type: 'telegram', url: 'https://t.me/analyst_job', description: 'Бизнес/системные аналитики', schedule: '09:00' } },
    { key: 'tg_evacuatejobs', value: { name: 'Remocate', type: 'telegram', url: 'https://t.me/evacuatejobs', description: 'Удалёнка и релокация из СНГ', schedule: '09:10' } },
    { key: 'tg_theyseeku', value: { name: 'Finder.work', type: 'telegram', url: 'https://t.me/theyseeku', description: 'Удалённая работа, до 10 вакансий/день', schedule: '09:20' } },
  ];
  for (const s of sources) {
    await invoke('memory_remember', { category: 'jobs_sources', key: s.key, value: JSON.stringify(s.value) }).catch(() => {});
  }
}
