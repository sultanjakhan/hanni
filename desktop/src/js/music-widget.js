// ── music-widget.js — YouTube Music widget (opens in Tauri WebView window) ──
import { invoke } from './state.js';

let mwDot = null;
let pollTimer = null;

async function refreshStatus() {
  try {
    const info = await invoke('get_youtube_music_info');
    if (mwDot) mwDot.classList.toggle('hidden', !info.playing);
  } catch (_) {}
}

async function openYTMusic() {
  try {
    await invoke('open_youtube_music');
    if (mwDot) mwDot.classList.remove('hidden');
  } catch (_) {}
}

export function initMusicWidget() {
  const existing = document.getElementById('music-widget');
  if (existing) existing.remove();

  const widget = document.createElement('div');
  widget.id = 'music-widget';
  widget.className = 'music-widget';
  widget.innerHTML = `
    <div class="mw-btn" id="mw-btn">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M9 18V5l12-2v13"/>
        <circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>
      </svg>
      <span class="mw-dot hidden" id="mw-dot"></span>
    </div>
  `;
  document.getElementById('content-area').appendChild(widget);

  mwDot = document.getElementById('mw-dot');
  document.getElementById('mw-btn').addEventListener('click', openYTMusic);

  refreshStatus();
  pollTimer = setInterval(refreshStatus, 30000);
}
