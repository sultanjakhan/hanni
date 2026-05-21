// ── wiki-markdown.js — [[wiki-links]] rendering for the dev wiki ──
// Wraps renderMarkdown: [[Имя]] / [[Имя|текст]] become internal links that
// resolve to a competency node by name. Plain [текст](url) links are
// untouched and keep opening in the browser.

import { renderMarkdown, escapeHtml } from './utils.js';

const WIKI_LINK_RE = /\[\[([^\]|\n]+?)(?:\|([^\]\n]+?))?\]\]/g;
const TOKEN_RE = /\{\{\{WIKI(\d+)\}\}\}/g;

/** Build a name→id lookup (case-insensitive) over competency nodes. */
export function buildNodeIndex(nodes) {
  const idx = new Map();
  for (const n of nodes || []) {
    if (n.kind === 'competency') {
      idx.set(String(n.name || '').trim().toLowerCase(), n.id);
    }
  }
  return idx;
}

/** Render markdown with [[wiki-links]] resolved against a competency index. */
export function renderWikiMarkdown(text, nodeIndex) {
  if (!text) return '';
  const links = [];
  // Stash wiki-links as brace tokens so markdown parsing leaves them intact.
  const staged = text.replace(WIKI_LINK_RE, (_, name, label) => {
    links.push({ target: name.trim(), display: (label || name).trim() });
    return `{{{WIKI${links.length - 1}}}}`;
  });
  return renderMarkdown(staged).replace(TOKEN_RE, (_, i) => {
    const { target, display } = links[Number(i)];
    const id = nodeIndex && nodeIndex.get(target.toLowerCase());
    if (id != null) {
      return `<a class="wiki-link" data-node-id="${id}">${escapeHtml(display)}</a>`;
    }
    return `<a class="wiki-link wiki-link-red" data-node-name="${escapeHtml(target)}"`
      + ` title="Компетенции «${escapeHtml(target)}» ещё нет">${escapeHtml(display)}</a>`;
  });
}
