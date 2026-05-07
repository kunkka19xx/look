let panel = null;
let timeEl = null;
let statusEl = null;

let state = 'idle'; // idle | work | break
let remaining = 0;
let interval = null;
const WORK = 25 * 60;
const BREAK = 5 * 60;

export function init() {
  panel = document.getElementById('cmd-panel-pomo');
  timeEl = document.getElementById('cmd-pomo-time');
  statusEl = document.getElementById('cmd-pomo-status');
}

export function enter() {
  panel.hidden = false;
  updateDisplay();
}

export function exit() {
  panel.hidden = true;
}

export function handleKey(e) {
  if (e.key === 'Enter') {
    e.preventDefault();
    toggle();
    return true;
  }
  if (e.key === 'r' || e.key === 'R') {
    e.preventDefault();
    reset();
    return true;
  }
  return true; // consume all keys in pomo
}

function toggle() {
  if (state === 'idle') {
    state = 'work';
    remaining = WORK;
    startTimer();
  } else if (interval) {
    clearInterval(interval);
    interval = null;
  } else {
    startTimer();
  }
  updateDisplay();
}

function reset() {
  if (interval) clearInterval(interval);
  interval = null;
  state = 'idle';
  remaining = 0;
  updateDisplay();
}

function startTimer() {
  if (interval) clearInterval(interval);
  interval = setInterval(() => {
    remaining--;
    if (remaining <= 0) {
      clearInterval(interval);
      interval = null;
      if (state === 'work') {
        state = 'break';
        remaining = BREAK;
        startTimer();
      } else {
        state = 'idle';
        remaining = 0;
      }
    }
    updateDisplay();
  }, 1000);
}

function updateDisplay() {
  if (state === 'idle') {
    timeEl.textContent = '25:00';
    statusEl.textContent = 'Press Enter to start focus session';
  } else {
    const m = Math.floor(remaining / 60);
    const s = remaining % 60;
    timeEl.textContent = `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    if (state === 'work') {
      statusEl.textContent = interval ? 'Focusing...' : 'Paused';
    } else {
      statusEl.textContent = interval ? 'Break time' : 'Paused (break)';
    }
  }
}
