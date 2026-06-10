// hh-sync.js — hh.ru/hh.kz only (see manifest matches). Auto-marks applications
// in Hanni: a vacancy page showing «Вы откликнулись» (or reaching that state
// after the user clicks «Откликнуться») sets stage 'applied'; the
// «Мои отклики» list page syncs negotiation outcomes (отказ/приглашение) back.
// Stage changes are upgrade-only — never move a vacancy back down the pipeline.

const HH_RANK = {
  found: 0, saved: 1, applied: 2, responded: 3,
  interview: 4, offer: 5, accepted: 6, rejected: 7, ignored: 8,
};
const hhPushed = new Map(); // url -> stage already pushed this page-visit

const hhApi = (path, method, body) =>
  new Promise((resolve) => chrome.runtime.sendMessage({ type: 'hanni-api', path, method, body }, resolve));

// almaty.hh.kz/vacancy/123?from=… → https://hh.kz/vacancy/123 (regional
// subdomains and tracking params would otherwise break URL-based dedup)
function hhNormalizeUrl(href) {
  try {
    const u = new URL(href, location.href);
    const host = u.hostname.match(/(?:^|\.)(hh\.(?:kz|ru))$/);
    const id = u.pathname.match(/\/vacancy\/(\d+)/);
    if (!host || !id) return null;
    return `https://${host[1]}/vacancy/${id[1]}`;
  } catch { return null; }
}

function hhToast(text) {
  let el = document.querySelector('.hanni-ext-toast');
  if (!el) {
    el = document.createElement('div');
    el.className = 'hanni-ext-toast';
    document.documentElement.appendChild(el);
  }
  el.textContent = text;
  el.classList.add('show');
  clearTimeout(el._hideTimer);
  el._hideTimer = setTimeout(() => el.classList.remove('show'), 4000);
}

async function hhUpsertStage(url, stage, fields = {}) {
  if (hhPushed.get(url) === stage) return 'kept';
  const cur = await hhApi(`/api/vacancy?url=${encodeURIComponent(url)}`, 'GET');
  if (!cur || cur.status === 0 || cur.status === 401) return 'error';
  const found = cur.ok && cur.data && cur.data.found;
  if (found && HH_RANK[cur.data.vacancy.stage] >= HH_RANK[stage]) {
    hhPushed.set(url, stage);
    return 'kept';
  }
  const res = await hhApi('/api/vacancy', 'POST', { url, stage, ...fields });
  if (!res || !res.ok) return 'error';
  hhPushed.set(url, stage);
  return found ? 'updated' : 'created';
}

// ── vacancy page: detect the applied state ────────────────────────────────

function hhAppliedVisible() {
  if (document.querySelector('[data-qa="vacancy-response-link-view-topic"]')) return true;
  const block = document.querySelector('[data-qa*="vacancy-response"]');
  const text = block ? block.parentElement.textContent : document.body.innerText.slice(0, 8000);
  return /вы откликнулись/i.test(text || '');
}

async function hhMarkApplied() {
  const url = hhNormalizeUrl(location.href);
  if (!url) return;
  const parsed = (window.__hanniParseJob && window.__hanniParseJob()) || {};
  const r = await hhUpsertStage(url, 'applied', {
    position: parsed.position || null,
    company: parsed.company || null,
    salary: parsed.salary || null,
    source: parsed.source || null,
  });
  if (r === 'created' || r === 'updated') hhToast('Hanni: отклик отмечен ✓');
}

function hhWatchVacancyPage() {
  if (hhAppliedVisible()) { hhMarkApplied(); return; }
  // hh hydrates late — one delayed re-check before relying on the click watcher
  setTimeout(() => { if (hhAppliedVisible()) hhMarkApplied(); }, 6000);
  document.addEventListener('click', (e) => {
    if (!e.target.closest('[data-qa="vacancy-response-link-top"]')) return;
    const started = Date.now();
    const timer = setInterval(() => {
      if (hhAppliedVisible()) { clearInterval(timer); hhMarkApplied(); }
      else if (Date.now() - started > 3 * 60 * 1000) clearInterval(timer);
    }, 2000);
  }, true);
}

// ── «Мои отклики»: sync outcomes back ─────────────────────────────────────

// A negotiation row always carries a status chip; vacancy links without one
// (e.g. «подходящие вакансии» recommendations) are skipped entirely.
function hhNegotiationStage(rowText) {
  if (/отказ/i.test(rowText)) return 'rejected';
  if (/приглашени/i.test(rowText)) return 'interview';
  if (/просмотрен/i.test(rowText)) return 'applied'; // «(не) просмотрено»
  return null;
}

async function hhSyncNegotiations() {
  const rows = new Map();
  for (const a of document.querySelectorAll('a[href*="/vacancy/"]')) {
    const url = hhNormalizeUrl(a.href);
    if (!url || rows.has(url)) continue;
    const row = a.closest('[data-qa*="negotiations-item"]') ||
                a.closest('li, article, tr') || a.parentElement;
    const stage = hhNegotiationStage(row ? row.textContent : '');
    if (!stage) continue;
    rows.set(url, { stage, title: a.textContent.trim().slice(0, 120) });
  }
  let changed = 0;
  for (const [url, info] of rows) {
    const r = await hhUpsertStage(url, info.stage, {
      position: info.title || null,
      source: location.hostname.replace(/^.*?(hh\.(?:kz|ru))$/, '$1'),
    });
    if (r === 'created' || r === 'updated') changed++;
  }
  if (changed) hhToast(`Hanni: обновлено откликов — ${changed}`);
}

function hhDebounce(fn, ms) {
  let t;
  return () => { clearTimeout(t); t = setTimeout(fn, ms); };
}

// ── router ────────────────────────────────────────────────────────────────

function hhInit() {
  if (/\/vacancy\/\d+/.test(location.pathname)) {
    setTimeout(hhWatchVacancyPage, 1500);
  } else if (location.pathname.startsWith('/applicant/negotiations')) {
    setTimeout(hhSyncNegotiations, 2000);
    // pagination / lazy lists re-render the DOM; rescan after things settle
    new MutationObserver(hhDebounce(hhSyncNegotiations, 2500))
      .observe(document.body, { childList: true, subtree: true });
  }
}

hhInit();
