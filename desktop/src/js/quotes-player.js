// ── quotes-player.js — Motivational quotes popup widget ──
import { randomQuote, getQuote } from './quotes-data.js';

let panel = null;

function renderQuote() {
  if (!panel) return;
  const q = getQuote();
  panel.querySelector('.qp-text').textContent = `"${q.text}"`;
  panel.querySelector('.qp-author').textContent = `— ${q.author}`;
  panel.querySelector('.qp-source').textContent = q.source;
}

function togglePanel() {
  if (!panel) return;
  const wasHidden = panel.classList.contains('hidden');
  if (wasHidden) randomQuote();
  panel.classList.toggle('hidden');
  if (wasHidden) renderQuote();
}

export function initQuotesPlayer() {
  const existing = document.getElementById('quotes-widget');
  if (existing) existing.remove();

  const widget = document.createElement('div');
  widget.id = 'quotes-widget';
  widget.className = 'quotes-widget';
  widget.innerHTML = `
    <div class="qp-panel hidden" id="qp-panel">
      <div class="qp-header">
        <span>Motivation</span>
      </div>
      <div class="qp-body">
        <div class="qp-text"></div>
        <div class="qp-author"></div>
        <div class="qp-source"></div>
      </div>
      <div class="qp-nav">
        <button class="qp-nav-btn" id="qp-prev">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M15 18l-6-6 6-6"/>
          </svg>
        </button>
        <button class="qp-nav-btn" id="qp-next">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
               stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 18l6-6-6-6"/>
          </svg>
        </button>
      </div>
    </div>
    <div class="qw-btn" id="qw-btn">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M14.5 17.5L3 6V3h3l11.5 11.5"/>
        <path d="M13 19l6-6"/><path d="M16 16l4 4"/><path d="M19 21l2-2"/>
      </svg>
    </div>
  `;
  document.getElementById('content-area').appendChild(widget);

  panel = document.getElementById('qp-panel');

  document.getElementById('qw-btn').addEventListener('click', togglePanel);
  document.getElementById('qp-prev').addEventListener('click', () => {
    randomQuote();
    renderQuote();
  });
  document.getElementById('qp-next').addEventListener('click', () => {
    randomQuote();
    renderQuote();
  });

  // Close on outside click
  document.addEventListener('click', (e) => {
    if (panel && !panel.classList.contains('hidden') && !widget.contains(e.target)) {
      panel.classList.add('hidden');
    }
  });
}
