// jobs-resume.js — Structured resume cards for Jobs Memory > Resume tab
import { invoke } from './state.js';
import { escapeHtml } from './utils.js';
import { showEditModal, EDIT_ICON } from './jobs-edit.js';

export async function renderResume(el) {
  let entries = [];
  try { entries = await invoke('memory_list', { category: 'jobs_resume', limit: 100 }); } catch {}

  if (entries.length === 0) {
    el.innerHTML = `<div class="jm-empty">
      <div class="jm-empty-icon">📄</div>
      <div class="jm-empty-title">Резюме не заполнено</div>
      <div class="jm-empty-desc">Добавьте данные вручную или нажмите кнопку ниже</div>
      <button class="btn btn-sm btn-primary jm-seed-btn">Заполнить из шаблона</button>
    </div>`;
    el.querySelector('.jm-seed-btn')?.addEventListener('click', async () => {
      await seedResumeData();
      await renderResume(el);
    });
    return;
  }

  const byKey = {};
  entries.forEach(e => { byKey[e.key] = JSON.parse(e.value || '{}'); });
  const sections = [];

  if (byKey.contact) {
    const c = byKey.contact;
    sections.push({ key: 'contact', html: `<div class="jm-section"><div class="jm-section-header"><span class="jm-section-title">Контакты</span><button class="jm-edit-btn" data-rkey="contact">${EDIT_ICON}</button></div>
      <div class="jm-contact-grid">
        ${cf('Имя', c.name)}${cf('Email', c.email)}${cf('Телефон', c.phone)}
        ${cf('Telegram', c.telegram)}${cf('LinkedIn', c.linkedin)}
      </div></div>` });
  }

  const expKeys = Object.keys(byKey).filter(k => k.startsWith('experience_')).sort();
  if (expKeys.length) {
    sections.push({ html: `<div class="jm-section"><div class="jm-section-title">Опыт работы</div>
      ${expKeys.map(k => expCard(k, byKey[k])).join('')}</div>` });
  }

  const projKeys = Object.keys(byKey).filter(k => k.startsWith('project_'));
  if (projKeys.length) {
    sections.push({ html: `<div class="jm-section"><div class="jm-section-title">Проекты</div>
      ${projKeys.map(k => expCard(k, byKey[k])).join('')}</div>` });
  }

  const eduKeys = Object.keys(byKey).filter(k => k.startsWith('education_'));
  if (eduKeys.length) {
    sections.push({ html: `<div class="jm-section"><div class="jm-section-title">Образование</div>
      ${eduKeys.map(k => eduCard(k, byKey[k])).join('')}</div>` });
  }

  if (byKey.skills) {
    const s = byKey.skills;
    sections.push({ key: 'skills', html: `<div class="jm-section"><div class="jm-section-header"><span class="jm-section-title">Навыки</span><button class="jm-edit-btn" data-rkey="skills">${EDIT_ICON}</button></div>
      <div class="jm-skills-grid">
        ${sr('Языки', s.languages)}${sr('Программирование', s.programming)}
        ${sr('Библиотеки', s.libraries)}${sr('Математика', s.math)}
        ${sr('Инструменты', s.tools)}
      </div></div>` });
  }

  el.innerHTML = `<div class="jm-resume">${sections.map(s => s.html).join('')}</div>`;
  wireEditButtons(el, byKey, () => renderResume(el));
}

function wireEditButtons(el, byKey, rerender) {
  // Contact
  el.querySelector('[data-rkey="contact"]')?.addEventListener('click', () => {
    const c = byKey.contact || {};
    showEditModal('jobs_resume', 'contact', {
      name: { label: 'Имя', value: c.name }, email: { label: 'Email', value: c.email },
      phone: { label: 'Телефон', value: c.phone }, telegram: { label: 'Telegram', value: c.telegram },
      linkedin: { label: 'LinkedIn', value: c.linkedin },
    }, rerender);
  });
  // Skills
  el.querySelector('[data-rkey="skills"]')?.addEventListener('click', () => {
    const s = byKey.skills || {};
    showEditModal('jobs_resume', 'skills', {
      languages: { label: 'Языки', value: s.languages }, programming: { label: 'Программирование', value: s.programming },
      libraries: { label: 'Библиотеки', value: s.libraries }, math: { label: 'Математика', value: s.math },
      tools: { label: 'Инструменты', value: s.tools },
    }, rerender);
  });
  // Experience / project / education cards
  el.querySelectorAll('[data-rkey^="experience_"], [data-rkey^="project_"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const d = byKey[btn.dataset.rkey] || {};
      showEditModal('jobs_resume', btn.dataset.rkey, {
        title: { label: 'Должность', value: d.title }, company: { label: 'Компания', value: d.company },
        dates: { label: 'Даты', value: d.dates }, bullets: { label: 'Достижения (по строке)', value: (d.bullets || []).join('\n'), type: 'textarea' },
      }, rerender);
    });
  });
  el.querySelectorAll('[data-rkey^="education_"]').forEach(btn => {
    btn.addEventListener('click', () => {
      const d = byKey[btn.dataset.rkey] || {};
      showEditModal('jobs_resume', btn.dataset.rkey, {
        degree: { label: 'Степень', value: d.degree || d.title }, school: { label: 'Учебное заведение', value: d.school },
        dates: { label: 'Даты', value: d.dates }, courses: { label: 'Курсы', value: d.courses },
      }, rerender);
    });
  });
}

function cf(label, val) {
  return val ? `<div class="jm-contact-field"><span class="jm-contact-label">${label}</span><span class="jm-contact-value">${escapeHtml(val)}</span></div>` : '';
}

function expCard(key, d) {
  const bullets = (d.bullets || []).map(b => `<li>${escapeHtml(b)}</li>`).join('');
  return `<div class="jm-card"><div class="jm-card-top"><span class="jm-card-title">${escapeHtml(d.title || '')}</span><span class="jm-card-dates">${escapeHtml(d.dates || '')}</span>
    <button class="jm-edit-btn" data-rkey="${key}">${EDIT_ICON}</button></div>
    <div class="jm-card-sub">${escapeHtml(d.company || d.description || '')}</div>
    ${bullets ? `<ul class="jm-card-bullets">${bullets}</ul>` : ''}</div>`;
}

function eduCard(key, d) {
  return `<div class="jm-card"><div class="jm-card-top"><span class="jm-card-title">${escapeHtml(d.degree || d.title || '')}</span><span class="jm-card-dates">${escapeHtml(d.dates || '')}</span>
    <button class="jm-edit-btn" data-rkey="${key}">${EDIT_ICON}</button></div>
    <div class="jm-card-sub">${escapeHtml(d.school || '')}</div>
    ${d.courses ? `<div class="jm-card-courses">${escapeHtml(d.courses)}</div>` : ''}</div>`;
}

function sr(label, val) {
  if (!val) return '';
  const tags = val.split(',').map(t => `<span class="jm-skill-tag">${escapeHtml(t.trim())}</span>`).join('');
  return `<div class="jm-skill-row"><span class="jm-skill-label">${label}</span><div class="jm-skill-tags">${tags}</div></div>`;
}

export async function seedResumeData() {
  const data = {
    contact: { name: 'Джаханов Султанбек', email: 'sultanbek.jakhanov@gmail.com', phone: '+7 776 069 5421', telegram: 'sultanjakhan', linkedin: 'sultanjakhan' },
    experience_1_kaspi: { title: 'Technical Project Manager', company: 'Kaspi.kz', dates: 'янв 2025 — мар 2025', bullets: ['Улучшение сервиса Robot в Kaspi Pay/2323 (партнерская часть)', 'Создал более 15 сценариев логики робота', 'Подняли ежемесячную эффективность на ~3%'] },
    experience_2_kbtu: { title: 'Ассистент профессора по БД', company: 'КБТУ', dates: 'сен 2024 — дек 2024', bullets: ['Подготовка и проведение практических занятий', 'Контроль и оценка 4 групп (100 человек)', 'Организация экзаменов и тестов'] },
    experience_3_kaztel: { title: 'Data Analyst', company: 'Kazakhtelecom', dates: 'июн 2022 — авг 2022', bullets: ['Карта 300+ потенциальных локаций для сотовых станций', 'Система true addresses для 40,000 клиентов', 'GIS, Nominatim, Yandex API (точность 80%+)', 'Сокращение времени анализа на 10%'] },
    project_hirex: { title: 'hirex.sh — IT Job Board для СНГ', description: '', bullets: ['MVP IT-джоб борда как альтернатива hh.ru', 'Matching-алгоритм, OAuth, фильтрация', 'Аудит безопасности: 10+ уязвимостей', 'React, Node.js, PostgreSQL, Drizzle ORM'] },
    education_1_kbtu: { degree: 'IT Менеджмент — Магистратура', school: 'КБТУ', dates: 'сен 2024 — июн 2026', courses: 'Cloud system, Data mining, System Analysis, Product management' },
    education_2_sdu: { degree: 'Мат. и комп. моделирование — Бакалавр', school: 'Университет им. Сулеймана Демиреля', dates: 'сен 2020 — июн 2024', courses: 'Programming, Math Analysis, Statistics, Linear Algebra, Databases, NLP, A/B Testing' },
    education_3_karpov: { title: 'Start Machine Learning', school: 'Karpov.Courses', dates: 'Онлайн', courses: 'Python, ML, Algorithms, Git, GitLab, Airflow, A/B, Statistics' },
    skills: { languages: 'Русский — родной, Казахский — родной, Английский — B2', programming: 'Python, SQL', libraries: 'Pandas, Folium, Scikit-learn, regex', math: 'Statistics, A/B testing', tools: 'Jira, Trello, Git, Gitlab, Google Cloud, Google Analytics, Office, UML, REST API, Power BI, Amplitude' },
  };
  for (const [key, value] of Object.entries(data)) {
    await invoke('memory_remember', { category: 'jobs_resume', key, value: JSON.stringify(value) }).catch(() => {});
  }
}
