// ── wiki-markdown.js — [[wiki-links]] rendering for the dev wiki ──
// Wraps renderMarkdown: [[Имя]] / [[Имя|текст]] become internal links that
// resolve to a dev skill (topic page) by name. Plain [текст](url) links are
// untouched and keep opening in the browser.

import { renderMarkdown, escapeHtml } from './utils.js';

const WIKI_LINK_RE = /\[\[([^\]|\n]+?)(?:\|([^\]\n]+?))?\]\]/g;
const TOKEN_RE = /\{\{\{WIKI(\d+)\}\}\}/g;

/** Build a name→id lookup (case-insensitive) from a list of dev skills. */
export function buildSkillIndex(skills) {
  const idx = new Map();
  for (const s of skills || []) {
    idx.set(String(s.name || '').trim().toLowerCase(), s.id);
  }
  return idx;
}

/** Render markdown with [[wiki-links]] resolved against skillIndex. */
export function renderWikiMarkdown(text, skillIndex) {
  if (!text) return '';
  const links = [];
  // Stash wiki-links as brace tokens so markdown parsing leaves them intact.
  const staged = text.replace(WIKI_LINK_RE, (_, name, label) => {
    links.push({ target: name.trim(), display: (label || name).trim() });
    return `{{{WIKI${links.length - 1}}}}`;
  });
  return renderMarkdown(staged).replace(TOKEN_RE, (_, i) => {
    const { target, display } = links[Number(i)];
    const id = skillIndex && skillIndex.get(target.toLowerCase());
    if (id != null) {
      return `<a class="wiki-link" data-skill-id="${id}">${escapeHtml(display)}</a>`;
    }
    return `<a class="wiki-link wiki-link-red" data-skill-name="${escapeHtml(target)}"`
      + ` title="Страница «${escapeHtml(target)}» ещё не создана">${escapeHtml(display)}</a>`;
  });
}
