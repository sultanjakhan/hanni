const { invoke } = window.__TAURI__.core;
const { listen, emit } = window.__TAURI__.event;

const pill = document.getElementById('pill');
const idleMsg = document.getElementById('idle-msg');
const titleEl = document.getElementById('overlay-title');
const timerEl = document.getElementById('overlay-timer');
let timerInterval = null;

function startTimer(startedAt) {
  if (timerInterval) clearInterval(timerInterval);
  const start = new Date(startedAt).getTime();
  function tick() {
    const elapsed = Math.floor((Date.now() - start) / 1000);
    const h = Math.floor(elapsed / 3600);
    const m = Math.floor((elapsed % 3600) / 60);
    const s = elapsed % 60;
    timerEl.textContent = h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
      : `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  tick();
  timerInterval = setInterval(tick, 1000);
}

function showActive(title, startedAt) {
  titleEl.textContent = title || 'Focus';
  startTimer(startedAt);
  pill.style.display = 'flex';
  idleMsg.classList.remove('visible');
}

function showIdle() {
  pill.style.display = 'none';
  idleMsg.classList.add('visible');
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
}

listen('focus-state', (event) => {
  const { active, title, started_at: startedAt } = event.payload;
  if (active) showActive(title, startedAt);
  else showIdle();
});

document.getElementById('overlay-stop').addEventListener('click', async () => {
  try {
    await invoke('stop_activity');
    emit('focus-state', { active: false });
    showIdle();
  } catch (error) {
    console.error(error);
  }
});

(async () => {
  try {
    const activity = await invoke('get_current_activity');
    if (activity && activity.id) {
      showActive(activity.title, activity.started_at);
    } else {
      showIdle();
    }
  } catch (_) {
    showIdle();
  }
})();
