// ── ambient-audio.js — Web Audio API engine for gapless ambient looping ──

export const SOUNDS = [
  { id: 'rain',        emoji: '\u{1F327}\u{FE0F}', name: '\u{0414}\u{043E}\u{0436}\u{0434}\u{044C}' },
  { id: 'forest',      emoji: '\u{1F332}',  name: '\u{041B}\u{0435}\u{0441}' },
  { id: 'ocean',       emoji: '\u{1F30A}',  name: '\u{041E}\u{043A}\u{0435}\u{0430}\u{043D}' },
  { id: 'fireplace',   emoji: '\u{1F525}',  name: '\u{041A}\u{043E}\u{0441}\u{0442}\u{0451}\u{0440}' },
  { id: 'wind',        emoji: '\u{1F4A8}',  name: '\u{0412}\u{0435}\u{0442}\u{0435}\u{0440}' },
  { id: 'birds',       emoji: '\u{1F426}',  name: '\u{041F}\u{0442}\u{0438}\u{0446}\u{044B}' },
  { id: 'thunder',     emoji: '\u{26C8}\u{FE0F}', name: '\u{0413}\u{0440}\u{043E}\u{0437}\u{0430}' },
  { id: 'white-noise', emoji: '\u{1F4FB}',  name: '\u{0411}\u{0435}\u{043B}\u{044B}\u{0439} \u{0448}\u{0443}\u{043C}' },
  { id: 'cafe',        emoji: '\u{2615}',   name: '\u{041A}\u{0430}\u{0444}\u{0435}' },
  { id: 'stream',      emoji: '\u{1F3DE}\u{FE0F}', name: '\u{0420}\u{0443}\u{0447}\u{0435}\u{0439}' },
  { id: 'piano',       emoji: '\u{1F3B9}',  name: '\u{041F}\u{0438}\u{0430}\u{043D}\u{043E}' },
  { id: 'guitar',      emoji: '\u{1F3B8}',  name: '\u{0413}\u{0438}\u{0442}\u{0430}\u{0440}\u{0430}' },
];

const STORAGE_KEY = 'hanni_ambient_state';
const FADE_SAMPLES = 200; // ~4.5ms at 44.1kHz — inaudible but eliminates loop clicks
const ctx = new AudioContext({ latencyHint: 'playback' });
ctx.addEventListener('statechange', () => {
  if (ctx.state === 'interrupted' || ctx.state === 'suspended') {
    ctx.resume().catch(() => {});
  }
});
const bufferCache = new Map();
const nodes = new Map();   // id → { source, gain, playing }
const volumes = {};
let masterVol = 0.8;
let onChange = null;

// Keep-alive: prevent macOS from suspending AudioContext
setInterval(() => {
  if (!isAnyPlaying()) return;
  if (ctx.state !== 'running') ctx.resume().catch(() => {});
}, 25_000);

function applyCrossfade(buf) {
  const len = FADE_SAMPLES;
  for (let ch = 0; ch < buf.numberOfChannels; ch++) {
    const d = buf.getChannelData(ch);
    const total = d.length;
    if (total < len * 2) continue;
    for (let i = 0; i < len; i++) {
      const t = i / len; // 0→1
      d[i] *= t;                   // fade-in
      d[total - 1 - i] *= t;      // fade-out
    }
  }
  return buf;
}

async function loadBuffer(id) {
  if (bufferCache.has(id)) return bufferCache.get(id);
  const resp = await fetch(`sounds/${id}.m4a`);
  const arr = await resp.arrayBuffer();
  const buf = await ctx.decodeAudioData(arr);
  applyCrossfade(buf);
  bufferCache.set(id, buf);
  return buf;
}

function getNode(id) {
  if (!nodes.has(id)) nodes.set(id, { source: null, gain: null, playing: false });
  return nodes.get(id);
}

async function startSound(id) {
  if (ctx.state === 'suspended') await ctx.resume();
  const node = getNode(id);
  if (node.playing) return;
  const buf = await loadBuffer(id);
  const gain = ctx.createGain();
  gain.gain.value = (volumes[id] ?? 0.7) * masterVol;
  gain.connect(ctx.destination);
  const source = ctx.createBufferSource();
  source.buffer = buf;
  source.loop = true;
  source.connect(gain);
  source.start();
  source.onended = () => {
    if (node.playing) {
      node.playing = false;
      node.source = null;
      node.gain = null;
      startSound(id).catch(() => {});
    }
  };
  node.source = source;
  node.gain = gain;
  node.playing = true;
}

function stopSound(id) {
  const node = nodes.get(id);
  if (!node || !node.playing) return;
  try { node.source.stop(); } catch (_) {}
  try { node.gain.disconnect(); } catch (_) {}
  node.source = null;
  node.gain = null;
  node.playing = false;
}

export async function toggle(id) {
  const node = getNode(id);
  if (node.playing) stopSound(id); else await startSound(id);
  save();
  onChange?.();
}

export function setVolume(id, v) {
  volumes[id] = v;
  const node = nodes.get(id);
  if (node?.gain) node.gain.gain.value = v * masterVol;
  save();
}

export function setMasterVolume(v) {
  masterVol = v;
  for (const [id, node] of nodes) {
    if (node.gain) node.gain.gain.value = (volumes[id] ?? 0.7) * masterVol;
  }
  save();
}

export function stopAll() {
  for (const id of nodes.keys()) stopSound(id);
  save();
  onChange?.();
}

export function isAnyPlaying() {
  for (const n of nodes.values()) if (n.playing) return true;
  return false;
}

export function isPlaying(id) { return getNode(id).playing; }
export function getVolume(id) { return volumes[id] ?? 0.7; }
export function getMasterVolume() { return masterVol; }
export function onStateChange(fn) { onChange = fn; }

function save() {
  const playing = [];
  for (const [id, n] of nodes) if (n.playing) playing.push(id);
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ playing, volumes, masterVol }));
}

export function restore() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return;
    const s = JSON.parse(raw);
    if (s.masterVol != null) masterVol = s.masterVol;
    if (s.volumes) Object.assign(volumes, s.volumes);
    for (const id of s.playing || []) startSound(id).catch(() => {});
  } catch (_) {}
  onChange?.();
}
