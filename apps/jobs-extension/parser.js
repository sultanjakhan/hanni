// parser.js — extracts job posting fields from the current page.
// Strategy: JSON-LD JobPosting → per-site selectors → generic fallback.
// Everything lands in editable panel fields, so best-effort is fine.

function hanniParseJsonLd() {
  for (const el of document.querySelectorAll('script[type="application/ld+json"]')) {
    let data;
    try { data = JSON.parse(el.textContent); } catch { continue; }
    const items = Array.isArray(data) ? data : (data['@graph'] || [data]);
    for (const item of items) {
      if (!item || item['@type'] !== 'JobPosting') continue;
      const org = item.hiringOrganization;
      let salary = '';
      const base = item.baseSalary;
      if (base && base.value) {
        const v = base.value;
        const cur = base.currency || v.currency || '';
        const parts = [];
        if (v.minValue) parts.push(`от ${v.minValue}`);
        if (v.maxValue) parts.push(`до ${v.maxValue}`);
        if (!parts.length && v.value) parts.push(String(v.value));
        salary = (parts.join(' ') + (cur ? ` ${cur}` : '')).trim();
      }
      return {
        position: String(item.title || '').trim(),
        company: String((typeof org === 'string' ? org : (org && org.name)) || '').trim(),
        salary,
      };
    }
  }
  return null;
}

// Per-site CSS selectors — checked after JSON-LD, fill whatever is missing.
const HANNI_SITE_RULES = {
  'hh.kz': {
    position: '[data-qa="vacancy-title"]',
    company: '[data-qa="vacancy-company-name"]',
    salary: '[data-qa="vacancy-salary"]',
  },
  'linkedin.com': {
    position: '.job-details-jobs-unified-top-card__job-title, .top-card-layout__title',
    company: '.job-details-jobs-unified-top-card__company-name, .topcard__org-name-link',
    salary: '',
  },
  'career.habr.com': {
    position: '.page-title__title',
    company: '.company_info .company_name a, .company_name',
    salary: '.basic-salary',
  },
  'djinni.co': {
    position: 'h1',
    company: 'a[href*="/jobs/company-"]',
    salary: '.public-salary-item',
  },
  'geekjob.ru': {
    position: 'h1',
    company: 'h5.company-name a, .company-name',
    salary: '.jobinfo .salary, .salary',
  },
  'vseti.app': { position: 'h1', company: '', salary: '' },
};
HANNI_SITE_RULES['hh.ru'] = HANNI_SITE_RULES['hh.kz'];

function hanniText(sel) {
  if (!sel) return '';
  const el = document.querySelector(sel);
  return el ? el.textContent.replace(/\s+/g, ' ').trim() : '';
}

function hanniSiteRules() {
  const host = location.hostname.replace(/^www\./, '');
  for (const [domain, rules] of Object.entries(HANNI_SITE_RULES)) {
    if (host === domain || host.endsWith('.' + domain)) return rules;
  }
  return null;
}

// og:title is commonly "Position — Company" or "Position | Company".
function hanniParseGeneric() {
  const og = document.querySelector('meta[property="og:title"]');
  const title = (og && og.content) || document.title || '';
  const m = title.split(/\s[—|–-]\s/);
  const h1 = hanniText('h1');
  const ogSite = document.querySelector('meta[property="og:site_name"]');
  return {
    position: h1 || (m[0] || '').trim(),
    company: (m[1] || (ogSite && ogSite.content) || '').trim(),
    salary: '',
  };
}

// Canonical URL: <link rel=canonical> → href without hash. Query is kept
// because some boards (LinkedIn) carry the job id in the query string.
function hanniJobUrl() {
  const canon = document.querySelector('link[rel="canonical"]');
  if (canon && canon.href) return canon.href;
  return location.href.split('#')[0];
}

function hanniLooksLikeJobPage() {
  if (hanniParseJsonLd()) return true;
  const rules = hanniSiteRules();
  if (rules && hanniText(rules.position)) return true;
  return /\/(vacanc|vacancy|vacancies|jobs?|career)/i.test(location.pathname);
}

// Public entry point used by content.js
window.__hanniParseJob = function () {
  const ld = hanniParseJsonLd() || {};
  const rules = hanniSiteRules() || {};
  const gen = hanniParseGeneric();
  return {
    position: ld.position || hanniText(rules.position) || gen.position || '',
    company: ld.company || hanniText(rules.company) || gen.company || '',
    salary: ld.salary || hanniText(rules.salary) || '',
    source: location.hostname.replace(/^www\./, ''),
    url: hanniJobUrl(),
    detected: hanniLooksLikeJobPage(),
  };
};
