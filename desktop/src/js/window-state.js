// window-state.js — DPI-correct window position/size persistence.
// tauri-plugin-window-state on macOS multi-monitor halves PhysicalPosition on
// restore (tao uses primary-monitor scale to convert). We disable POSITION/SIZE
// flags on the plugin (lib.rs) and handle them here using LogicalPosition.

const KEY_PROD = 'hanni_window_state';
const KEY_DEV = 'hanni_window_state_dev';

function getKey() {
  // Dev (cargo tauri dev) and prod use the same bundle id and may share a
  // webview storage origin. Pick a different key per build so they don't
  // overwrite each other.
  return (window.__HANNI_DEV__ === true) ? KEY_DEV : KEY_PROD;
}

async function getWin() {
  return window.__TAURI__.window.getCurrentWindow();
}

async function readState() {
  const w = await getWin();
  const sf = await w.scaleFactor();
  const op = await w.outerPosition();
  const os = await w.outerSize();
  const lp = op.toLogical(sf);
  const ls = os.toLogical(sf);
  return { x: lp.x, y: lp.y, w: ls.width, h: ls.height };
}

async function save() {
  try {
    const s = await readState();
    if (!Number.isFinite(s.w) || !Number.isFinite(s.h) || s.w < 1 || s.h < 1) return;
    localStorage.setItem(getKey(), JSON.stringify(s));
  } catch (_) {}
}

async function restore() {
  try {
    const raw = localStorage.getItem(getKey());
    if (!raw) return;
    const s = JSON.parse(raw);
    const w = await getWin();
    const { LogicalPosition, LogicalSize } = window.__TAURI__.window;
    if (Number.isFinite(s.w) && Number.isFinite(s.h) && s.w >= 1 && s.h >= 1) {
      await w.setSize(new LogicalSize(s.w, s.h));
    }
    if (Number.isFinite(s.x) && Number.isFinite(s.y)) {
      await w.setPosition(new LogicalPosition(s.x, s.y));
    }
  } catch (_) {}
}

let saveTimer = null;
function scheduleSave() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(save, 400);
}

export async function initWindowState(isDev) {
  window.__HANNI_DEV__ = isDev === true;
  await restore();
  const w = await getWin();
  await w.onMoved(scheduleSave);
  await w.onResized(scheduleSave);
  // Final save on hide/blur — covers cmd+Q which may not fire move/resize.
  window.addEventListener('beforeunload', save);
}
