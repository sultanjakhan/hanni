// ── ambient-audio.js — Web Audio API engine for gapless ambient looping ──

export const SOUNDS = [
  { id: 'rain',        emoji: '\u{1F327}\u{FE0F}', name: '\u{0414}\u{043E}\u{0436}\u{0434}\u{044C}' },
  { id: 'forest',      emoji: '\u{1F332}',  name: '\u{041B}\u{0435}\u{0441}' },
  { id: 'ocean',       emoji: '\u{1F30A}',  name: '\u{041E}\u{043A}\u{0435}\u{0430}\u{043D}' },
  { id: 'fireplace',   emoji: '\u{1F525}',  name: '\u{041A}\u{043E}\u{0441}\u{0442}\u{0451}\u{0440}' },
  { id: 'wind',        emoji: '\u{1F4A8}',  name: '\u{0412}\u{0435}\u{0442}\u{0435}\u{0440}' },
  { id: 'birds',       emoji: '\u{1F426}',  name: '\u{041F}\u{0442}\u{0438}\u{0446}\u{044B}' },
  { id: 'thunder',     emoji: '\u{26C8}\u{FE0F}', name: '\u{0413}\u{0440}\u{043E}\u{0437}\u{0430}' },
  { id: 'stream',      emoji: '\u{1F3DE}\u{FE0F}', name: '\u{0420}\u{0443}\u{0447}\u{0435}\u{0439}' },
  { id: 'jupiter',     emoji: '\u{1FA90}',  name: '\u{042E}\u{043F}\u{0438}\u{0442}\u{0435}\u{0440}' },
  { id: 'saturn',      emoji: '\u{1F30C}',  name: '\u{0421}\u{0430}\u{0442}\u{0443}\u{0440}\u{043D}' },
  { id: 'blackhole',   emoji: '\u{1F573}\u{FE0F}',  name: '\u{0427}\u{0451}\u{0440}\u{043D}\u{0430}\u{044F} \u{0434}\u{044B}\u{0440}\u{0430}' },
  { id: 'space',       emoji: '\u{1F680}',  name: '\u{041A}\u{043E}\u{0441}\u{043C}\u{043E}\u{0441}' },
];

const STORAGE_KEY = 'hanni_ambient_state';

// Lazy AudioContext: WebKit on macOS spawns new AudioContext in 'suspended' state
// until a user gesture. Creating it inside the first click handler sidesteps that.
let ctx = null;
function getCtx() {
  if (ctx) return ctx;
  ctx = new AudioContext({ latencyHint: 'playback' });
  ctx.addEventListener('statechange', () => {
    if (!isAnyPlaying()) return;
    if (ctx.state === 'interrupted' || ctx.state === 'suspended') {
      ctx.resume().catch(() => {});
    }
  });
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible') return;
    if (!isAnyPlaying()) return;
    if (ctx.state !== 'running') ctx.resume().catch(() => {});
  });
  return ctx;
}

const bufferCache = new Map();
const nodes = new Map();   // id → { source, gain, playing }
const volumes = {};
let masterVol = 0.8;
let onChange = null;

async function loadBuffer(id) {
  if (bufferCache.has(id)) return bufferCache.get(id);
  const resp = await fetch(`sounds/${id}.m4a`);
  const arr = await resp.arrayBuffer();
  const buf = await getCtx().decodeAudioData(arr);
  bufferCache.set(id, buf);
  return buf;
}

function getNode(id) {
  if (!nodes.has(id)) nodes.set(id, { source: null, gain: null, playing: false, starting: false });
  return nodes.get(id);
}

async function startSound(id) {
  // Create/resume ctx synchronously inside click handler — before any await —
  // so WebKit accepts this as a valid user-gesture activation.
  const c = getCtx();
  if (c.state !== 'running') c.resume().catch(() => {});
  const node = getNode(id);
  if (node.playing || node.starting) return;
  node.starting = true;
  try {
    const buf = await loadBuffer(id);
    if (!node.starting) return; // cancelled by stop during loadBuffer
    const gain = c.createGain();
    gain.gain.value = (volumes[id] ?? 0.7) * masterVol;
    gain.connect(c.destination);
    const source = c.createBufferSource();
    source.buffer = buf;
    source.loop = true;
    source.connect(gain);
    source.start();
    node.source = source;
    node.gain = gain;
    node.playing = true;
  } finally {
    node.starting = false;
  }
}

function stopSound(id) {
  const node = nodes.get(id);
  if (!node) return;
  node.starting = false; // cancel any in-flight startSound
  if (!node.playing) return;
  try { node.source.stop(); } catch (_) {}
  try { node.source.disconnect(); } catch (_) {}
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
