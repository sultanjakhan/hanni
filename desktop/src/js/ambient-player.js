// ── ambient-player.js — Ambient sound mixer popup widget ──
import {
  SOUNDS, toggle, setVolume, setMasterVolume, stopAll,
  isAnyPlaying, isPlaying, getVolume, getMasterVolume,
  onStateChange, restore
} from './ambient-audio.js';

let panel = null;
let dot = null;

function updateDot() {
  if (dot) dot.classList.toggle('hidden', !isAnyPlaying());
}

function updateButtons() {
  if (!panel) return;
  for (const s of SOUNDS) {
    const btn = panel.querySelector(`.ap-play[data-id="${s.id}"]`);
    if (btn) {
      const playing = isPlaying(s.id);
      btn.classList.toggle('playing', playing);
      btn.textContent = playing ? '⏸' : '▶';
    }
  }
  updateDot();
}

function buildList() {
  return SOUNDS.map(s => `
    <div class="ap-item" data-id="${s.id}">
      <span class="ap-icon">${s.emoji}</span>
      <span class="ap-name">${s.name}</span>
      <input type="range" class="ap-vol" data-id="${s.id}"
             min="0" max="100" value="${Math.round(getVolume(s.id) * 100)}">
      <button class="ap-play${isPlaying(s.id) ? ' playing' : ''}" data-id="${s.id}">
        ${isPlaying(s.id) ? '⏸' : '▶'}
      </button>
    </div>
  `).join('');
}

function togglePanel() {
  if (!panel) return;
  panel.classList.toggle('hidden');
}

export function initAmbientPlayer() {
  const existing = document.getElementById('music-widget');
  if (existing) existing.remove();

  const widget = document.createElement('div');
  widget.id = 'music-widget';
  widget.className = 'music-widget';
  widget.innerHTML = `
    <div class="ap-panel hidden" id="ap-panel">
      <div class="ap-header">
        <span>Ambient</span>
        <button class="ap-stop-all" id="ap-stop-all">Stop All</button>
      </div>
      <div class="ap-master">
        <span class="ap-master-label">Master</span>
        <input type="range" class="ap-slider" id="ap-master-vol"
               min="0" max="100" value="${Math.round(getMasterVolume() * 100)}">
      </div>
      <div class="ap-list" id="ap-list">${buildList()}</div>
    </div>
    <div class="mw-btn" id="mw-btn">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M9 18V5l12-2v13"/>
        <circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
      </svg>
      <span class="mw-dot hidden" id="mw-dot"></span>
    </div>
  `;
  document.getElementById('content-area').appendChild(widget);

  panel = document.getElementById('ap-panel');
  dot = document.getElementById('mw-dot');

  document.getElementById('mw-btn').addEventListener('click', togglePanel);
  document.getElementById('ap-stop-all').addEventListener('click', () => {
    stopAll();
    updateButtons();
  });
  document.getElementById('ap-master-vol').addEventListener('input', (e) => {
    setMasterVolume(e.target.value / 100);
  });

  // Event delegation for list
  const list = document.getElementById('ap-list');
  list.addEventListener('click', async (e) => {
    const btn = e.target.closest('.ap-play');
    if (!btn) return;
    await toggle(btn.dataset.id);
    updateButtons();
  });
  list.addEventListener('input', (e) => {
    if (!e.target.classList.contains('ap-vol')) return;
    setVolume(e.target.dataset.id, e.target.value / 100);
  });

  // Close on outside click
  document.addEventListener('click', (e) => {
    if (panel && !panel.classList.contains('hidden') && !widget.contains(e.target)) {
      panel.classList.add('hidden');
    }
  });

  onStateChange(updateButtons);
  restore();
  updateDot();
}
