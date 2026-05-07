// --- Default sessions ---
const DEFAULT_SESSIONS = [
  { type: 'focus', duration: 30, name: 'Deep Work' },
  { type: 'break', duration: 5, name: 'Short Break' },
  { type: 'focus', duration: 30, name: 'Review' },
  { type: 'break', duration: 5, name: 'Short Break' },
  { type: 'focus', duration: 30, name: 'Wrap Up' },
  { type: 'break', duration: 15, name: 'Long Break' },
];

const ENDING_SOON_SECS = 10;
const IDLE_FADE_SECS = 5;

// --- Persistence ---
const STORAGE_KEY_SESSIONS = 'pomo_sessions';
const STORAGE_KEY_STYLE = 'pomo_timer_style';

function loadConfig() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_SESSIONS);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) return parsed;
    }
  } catch {}
  return DEFAULT_SESSIONS.map((s) => ({ ...s }));
}

function saveConfig() {
  try {
    localStorage.setItem(STORAGE_KEY_SESSIONS, JSON.stringify(sessions));
    localStorage.setItem(STORAGE_KEY_STYLE, timerStyle);
  } catch {}
}

function loadStyle() {
  return localStorage.getItem(STORAGE_KEY_STYLE) || 'modern';
}

// --- State ---
let sessions = loadConfig();
let activeIndex = null; // null = idle
let secondsLeft = 0;
let running = false;
let lastTickAt = null;
let tickTimer = null;
let timerStyle = loadStyle();
let idleFaded = false;
let idleFadeTimer = null;
let endingSoonFired = false;
let sessionsOpen = false;
let settingsOpen = false;

// --- DOM refs ---
let panel, header, sessionNameEl, canvas, ctx;
let toggleBtn, skipBtn, resetBtn, settingsBtn;
let settingsPanel, sessionsListEl, sessionsActionsEl, chevronEl;
let cardEl, controlsEl, sessionsEl;

export function init() {
  panel = document.getElementById('cmd-panel-pomo');
  header = document.getElementById('cmd-pomo-header');
  sessionNameEl = document.getElementById('cmd-pomo-session-name');
  canvas = document.getElementById('cmd-pomo-canvas');
  ctx = canvas.getContext('2d');
  cardEl = document.getElementById('cmd-pomo-card');
  controlsEl = document.getElementById('cmd-pomo-controls');

  toggleBtn = document.getElementById('cmd-pomo-toggle');
  skipBtn = document.getElementById('cmd-pomo-skip');
  resetBtn = document.getElementById('cmd-pomo-reset');
  settingsBtn = document.getElementById('cmd-pomo-settings-btn');
  settingsPanel = document.getElementById('cmd-pomo-settings');

  sessionsEl = document.getElementById('cmd-pomo-sessions');
  sessionsListEl = document.getElementById('cmd-pomo-sessions-list');
  sessionsActionsEl = document.getElementById('cmd-pomo-sessions-actions');
  chevronEl = document.getElementById('cmd-pomo-chevron');

  // Button events
  toggleBtn.addEventListener('click', toggle);
  skipBtn.addEventListener('click', skip);
  resetBtn.addEventListener('click', reset);
  settingsBtn.addEventListener('click', toggleSettings);

  // Settings style buttons
  settingsPanel.querySelectorAll('.cmd-pomo-style-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      timerStyle = btn.dataset.style;
      updateStyleButtons();
      drawTimer();
      saveConfig();
    });
  });

  // Session list toggle
  document.getElementById('cmd-pomo-sessions-toggle').addEventListener('click', toggleSessionsList);

  // Add session buttons (stop propagation to avoid triggering toggle)
  document.getElementById('cmd-pomo-add-focus').addEventListener('click', (e) => { e.stopPropagation(); addSession('focus'); });
  document.getElementById('cmd-pomo-add-break').addEventListener('click', (e) => { e.stopPropagation(); addSession('break'); });

  // Idle restore events
  panel.addEventListener('click', restoreFromIdle);
  panel.addEventListener('wheel', restoreFromIdle);
}

export function enter() {
  panel.hidden = false;
  updateAll();
  startTick();
}

export function exit() {
  panel.hidden = true;
  stopTick();
  // Restore idle fade if active
  if (idleFaded) {
    idleFaded = false;
    applyIdleFade();
  }
  clearIdleFade();
}

export function handleKey(e) {
  if (e.key === ' ' && !e.metaKey && !e.ctrlKey) {
    e.preventDefault();
    toggle();
    return true;
  }
  if (e.key === 'r' || e.key === 'R') {
    e.preventDefault();
    reset();
    return true;
  }
  return false;
}

// --- Timer control ---

function toggle() {
  if (activeIndex === null) {
    // Start first session
    activeIndex = 0;
    secondsLeft = sessions[0].duration * 60;
    running = true;
    endingSoonFired = false;
    lastTickAt = Date.now();
    startIdleFade();
  } else if (running) {
    // Pause
    running = false;
    clearIdleFade();
    restoreFromIdle();
  } else {
    // Resume
    running = true;
    lastTickAt = Date.now();
    startIdleFade();
  }
  updateAll();
}

function skip() {
  if (activeIndex === null) return;
  advanceSession();
}

function reset() {
  running = false;
  activeIndex = null;
  secondsLeft = 0;
  endingSoonFired = false;
  clearIdleFade();
  restoreFromIdle();
  updateAll();
}

function advanceSession() {
  const next = activeIndex + 1;
  if (next < sessions.length) {
    activeIndex = next;
    secondsLeft = sessions[next].duration * 60;
    running = true;
    endingSoonFired = false;
    lastTickAt = Date.now();
    notifyPhaseComplete(next - 1, next);
    startIdleFade();
  } else {
    notifyPhaseComplete(activeIndex, null);
    reset();
  }
  updateAll();
}

// --- Tick (wall-clock) ---

function startTick() {
  stopTick();
  tickTimer = setInterval(tick, 500);
}

function stopTick() {
  if (tickTimer) {
    clearInterval(tickTimer);
    tickTimer = null;
  }
}

function tick() {
  if (!running || activeIndex === null) return;

  const now = Date.now();
  const elapsed = Math.floor((now - lastTickAt) / 1000);
  if (elapsed <= 0) return;

  lastTickAt = now;
  secondsLeft = Math.max(0, secondsLeft - elapsed);

  // Ending soon notification
  if (!endingSoonFired && secondsLeft <= ENDING_SOON_SECS && secondsLeft > 0) {
    endingSoonFired = true;
    notifyEndingSoon();
  }

  // Phase complete
  if (secondsLeft <= 0) {
    advanceSession();
    return;
  }

  drawTimer();
}

// --- Notifications ---

function notifyEndingSoon() {
  const s = sessions[activeIndex];
  const label = s.type === 'focus' ? 'Focus ending soon' : 'Break ending soon';
  notify(label, `${secondsLeft}s remaining`);
}

function notifyPhaseComplete(doneIdx, nextIdx) {
  const done = sessions[doneIdx];
  const title = done.type === 'focus' ? 'Focus done' : 'Break done';
  const body = nextIdx !== null
    ? `Next: ${sessions[nextIdx].name} (${sessions[nextIdx].duration}m)`
    : 'All sessions complete!';
  notify(title, body);
}

function notify(title, body) {
  if ('Notification' in window && Notification.permission === 'granted') {
    new Notification(title, { body });
  } else if ('Notification' in window && Notification.permission === 'default') {
    Notification.requestPermission();
  }
}

// --- Idle fade ---

function startIdleFade() {
  clearIdleFade();
  idleFadeTimer = setTimeout(() => {
    idleFaded = true;
    applyIdleFade();
  }, IDLE_FADE_SECS * 1000);
}

function clearIdleFade() {
  if (idleFadeTimer) {
    clearTimeout(idleFadeTimer);
    idleFadeTimer = null;
  }
}

function restoreFromIdle() {
  if (!idleFaded) return;
  idleFaded = false;
  applyIdleFade();
  if (running) startIdleFade();
}

function applyIdleFade() {
  const sidebar = document.getElementById('cmd-sidebar');
  const divider = document.querySelector('.cmd-divider');
  const headerBar = panel.querySelector('.cmd-header-bar');

  controlsEl.classList.toggle('cmd-pomo-fade-out', idleFaded);
  sessionsEl.classList.toggle('cmd-pomo-fade-out', idleFaded);
  cardEl.classList.toggle('pomo-idle-expanded', idleFaded);
  panel.classList.toggle('pomo-standby', idleFaded);

  if (sidebar) sidebar.classList.toggle('cmd-pomo-fade-out', idleFaded);
  if (divider) divider.classList.toggle('cmd-pomo-fade-out', idleFaded);
  if (headerBar) headerBar.classList.toggle('cmd-pomo-fade-out', idleFaded);

  drawTimer(); // redraw at new size
}

// --- Rendering ---

function updateAll() {
  updateHeader();
  updateSessionName();
  updateButtons();
  updateSessionList();
  drawTimer();
}

function updateHeader() {
  if (activeIndex !== null && running) {
    header.textContent = `Running: ${sessions[activeIndex].name}`;
  } else if (activeIndex !== null) {
    header.textContent = `Paused: ${sessions[activeIndex].name}`;
  } else {
    header.textContent = `${sessions.length} sessions planned`;
  }
}

function updateSessionName() {
  if (activeIndex !== null) {
    sessionNameEl.textContent = sessions[activeIndex].name;
  } else {
    sessionNameEl.textContent = '';
  }
}

function updateButtons() {
  if (activeIndex === null) {
    toggleBtn.textContent = 'Start';
    toggleBtn.className = 'cmd-pomo-btn cmd-pomo-btn-toggle';
    skipBtn.hidden = true;
    resetBtn.hidden = true;
  } else if (running) {
    toggleBtn.textContent = 'Pause';
    toggleBtn.className = 'cmd-pomo-btn cmd-pomo-btn-toggle pomo-running';
    skipBtn.hidden = false;
    resetBtn.hidden = false;
  } else {
    toggleBtn.textContent = 'Resume';
    toggleBtn.className = 'cmd-pomo-btn cmd-pomo-btn-toggle pomo-paused';
    skipBtn.hidden = false;
    resetBtn.hidden = false;
  }
}

function updateStyleButtons() {
  settingsPanel.querySelectorAll('.cmd-pomo-style-btn').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.style === timerStyle);
  });
}

function toggleSettings() {
  settingsOpen = !settingsOpen;
  settingsPanel.style.display = settingsOpen ? '' : 'none';
  if (settingsOpen) updateStyleButtons();
}

// --- Timer drawing ---

function drawTimer() {
  const dpr = window.devicePixelRatio || 1;
  const size = idleFaded ? 240 : 180;
  canvas.width = size * dpr;
  canvas.height = size * dpr;
  canvas.style.width = size + 'px';
  canvas.style.height = size + 'px';
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, size, size);

  const totalSecs = activeIndex !== null ? sessions[activeIndex].duration * 60 : sessions[0].duration * 60;
  const progress = activeIndex !== null ? 1 - secondsLeft / totalSecs : 0;
  const color = getTimerColor();

  switch (timerStyle) {
    case 'modern': drawModernRing(size, progress, color, totalSecs); break;
    case 'vintage': drawVintageDial(size, progress, color); break;
    case 'minimal': drawMinimalText(size, progress, color, totalSecs); break;
    default: drawModernRing(size, progress, color, totalSecs);
  }
}

function getTimerColor() {
  if (activeIndex === null) return getComputedStyle(document.documentElement).getPropertyValue('--accent-color').trim() || '#6b8afd';
  return sessions[activeIndex].type === 'focus'
    ? (getComputedStyle(document.documentElement).getPropertyValue('--color-danger').trim() || '#e55')
    : (getComputedStyle(document.documentElement).getPropertyValue('--color-success').trim() || '#3a3');
}

function formatTime(secs) {
  if (secs <= 0 && activeIndex === null) {
    secs = sessions[0].duration * 60;
  }
  const m = Math.floor(Math.max(0, secs) / 60);
  const s = Math.max(0, secs) % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

function drawModernRing(size, progress, color, totalSecs) {
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 12;
  const lw = 6;

  // Background ring
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.strokeStyle = 'rgba(128,128,128,0.15)';
  ctx.lineWidth = lw;
  ctx.stroke();

  // Progress ring
  if (progress > 0) {
    ctx.beginPath();
    ctx.arc(cx, cy, r, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * progress);
    ctx.strokeStyle = color;
    ctx.lineWidth = lw;
    ctx.lineCap = 'round';
    ctx.stroke();
  }

  // Time text
  ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--font-color').trim() || '#fff';
  ctx.font = `bold ${Math.round(size * 0.22)}px monospace`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(formatTime(secondsLeft), cx, cy);
}

function drawVintageDial(size, progress, color) {
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 10;

  // Tick marks
  for (let i = 0; i < 60; i++) {
    const angle = (i / 60) * Math.PI * 2 - Math.PI / 2;
    const isMajor = i % 5 === 0;
    const outerR = r;
    const innerR = r - (isMajor ? 12 : 6);
    ctx.beginPath();
    ctx.moveTo(cx + Math.cos(angle) * innerR, cy + Math.sin(angle) * innerR);
    ctx.lineTo(cx + Math.cos(angle) * outerR, cy + Math.sin(angle) * outerR);
    ctx.strokeStyle = 'rgba(128,128,128,0.3)';
    ctx.lineWidth = isMajor ? 2 : 1;
    ctx.stroke();
  }

  // Center hub
  ctx.beginPath();
  ctx.arc(cx, cy, 4, 0, Math.PI * 2);
  ctx.fillStyle = color;
  ctx.fill();

  // Needle
  const needleAngle = -Math.PI / 2 + Math.PI * 2 * progress;
  const needleLen = r - 20;
  ctx.beginPath();
  ctx.moveTo(cx, cy);
  ctx.lineTo(cx + Math.cos(needleAngle) * needleLen, cy + Math.sin(needleAngle) * needleLen);
  ctx.strokeStyle = color;
  ctx.lineWidth = 2.5;
  ctx.lineCap = 'round';
  ctx.stroke();
}

function drawMinimalText(size, progress, color, totalSecs) {
  const cx = size / 2;
  const fontColor = getComputedStyle(document.documentElement).getPropertyValue('--font-color').trim() || '#fff';

  // Large time
  ctx.fillStyle = fontColor;
  ctx.font = `800 ${Math.round(size * 0.32)}px monospace`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(formatTime(secondsLeft), cx, size * 0.42);

  // Progress bar
  const barW = size * 0.8;
  const barH = 4;
  const barX = (size - barW) / 2;
  const barY = size * 0.68;

  // Background
  ctx.beginPath();
  ctx.roundRect(barX, barY, barW, barH, barH / 2);
  ctx.fillStyle = 'rgba(128,128,128,0.15)';
  ctx.fill();

  // Fill
  if (progress > 0) {
    ctx.beginPath();
    ctx.roundRect(barX, barY, barW * progress, barH, barH / 2);
    ctx.fillStyle = color;
    ctx.fill();
  }
}

// --- Session list UI ---

function toggleSessionsList() {
  sessionsOpen = !sessionsOpen;
  sessionsListEl.style.display = sessionsOpen ? '' : 'none';
  chevronEl.classList.toggle('open', sessionsOpen);
  if (sessionsOpen) renderSessionList();
}

function updateSessionList() {
  if (sessionsOpen) renderSessionList();
}

function renderSessionList() {
  sessionsListEl.innerHTML = '';
  sessions.forEach((s, i) => {
    const row = document.createElement('div');
    row.className = `cmd-pomo-session-row ${activeIndex !== null && i < activeIndex ? 'past' : ''}`;

    // Dot
    const dot = document.createElement('span');
    dot.className = `cmd-pomo-session-dot ${s.type}`;
    row.appendChild(dot);

    // Name input
    const nameInput = document.createElement('input');
    nameInput.className = 'cmd-pomo-session-name-input';
    nameInput.value = s.name;
    nameInput.addEventListener('change', () => {
      sessions[i].name = nameInput.value.trim() || s.name;
      saveConfig();
      updateAll();
    });
    row.appendChild(nameInput);

    // Duration input
    const durInput = document.createElement('input');
    durInput.className = 'cmd-pomo-session-dur';
    durInput.type = 'number';
    durInput.min = '1';
    durInput.max = '120';
    durInput.value = s.duration;
    durInput.addEventListener('change', () => {
      const val = parseInt(durInput.value);
      if (val > 0 && val <= 120) sessions[i].duration = val;
      saveConfig();
      updateAll();
    });
    row.appendChild(durInput);

    // Type toggle
    const typeBtn = document.createElement('button');
    typeBtn.className = 'cmd-pomo-session-type-btn';
    typeBtn.textContent = s.type === 'focus' ? 'F' : 'B';
    typeBtn.addEventListener('click', () => {
      sessions[i].type = s.type === 'focus' ? 'break' : 'focus';
      updateAll();
    });
    row.appendChild(typeBtn);

    // Delete
    const delBtn = document.createElement('button');
    delBtn.className = 'cmd-pomo-session-del';
    delBtn.textContent = '\u00D7';
    delBtn.addEventListener('click', () => {
      sessions.splice(i, 1);
      if (activeIndex !== null && activeIndex >= sessions.length) {
        reset();
      }
      saveConfig();
      updateAll();
    });
    row.appendChild(delBtn);

    sessionsListEl.appendChild(row);
  });
}

function addSession(type) {
  const name = type === 'focus' ? 'Focus' : 'Break';
  const duration = type === 'focus' ? 30 : 5;
  sessions.push({ type, duration, name });
  saveConfig();
  updateAll();
}
