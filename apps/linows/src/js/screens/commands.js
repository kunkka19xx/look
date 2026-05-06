const COMMANDS = [
  { id: 'calc', label: '/calc', shortcut: '1', detail: 'Evaluate math...', icon: 'f(x)' },
  { id: 'pomo', label: '/pomo', shortcut: '2', detail: 'Pomodoro focus...', icon: '\u23F1' },
  { id: 'kill', label: '/kill', shortcut: '3', detail: 'Force kill app...', icon: '\u2718' },
  { id: 'shell', label: '/shell', shortcut: '4', detail: 'Run a shell co...', icon: '\u25B8' },
  { id: 'sys', label: '/sys', shortcut: '5', detail: 'Show system in...', icon: 'i' },
];

const ACCEPTS_INPUT = new Set(['calc', 'shell', 'kill']);

const ICON_MAP = {
  calc: 'f(x)',
  pomo: '\u23F1',
  kill: '\u2718',
  shell: '\u25B8',
  sys: '\u24D8',
};

let container = null;
let mainSearchInput = null;
let cmdInput = null;
let active = false;
let activeCommandId = 'calc';
let selectedIndex = 0;
let onExit = null;
let onExecute = null;
let onCommandChange = null;
let getIconFn = null;
let savedChildren = []; // original container children (results-list, preview-panel)

// Kill state
let processList = [];
let filteredProcesses = [];
let processSelectedIndex = 0;
let killConfirmPid = null;

// Pomo state
let pomoState = 'idle'; // idle | work | break
let pomoRemaining = 0; // seconds
let pomoInterval = null;
const POMO_WORK = 25 * 60;
const POMO_BREAK = 5 * 60;

// Sys state
let sysInfoSections = null;

export function init(containerEl, inputEl, { onExitMode, onExecuteCommand, onGetIcon }) {
  container = containerEl;
  mainSearchInput = inputEl;
  onExit = onExitMode;
  onExecute = onExecuteCommand;
  getIconFn = onGetIcon || null;
}

export function setOnCommandChange(fn) {
  onCommandChange = fn;
}

export function isActive() {
  return active;
}

export function enter() {
  active = true;
  selectedIndex = COMMANDS.findIndex((c) => c.id === activeCommandId);
  if (selectedIndex < 0) selectedIndex = 0;

  // Save original children before replacing content
  savedChildren = [...container.childNodes];
  render();
  autoRun();
}

export function exit() {
  active = false;
  killConfirmPid = null;

  // Remove command-mode elements and restore original children
  container.innerHTML = '';
  savedChildren.forEach((child) => container.appendChild(child));
  savedChildren = [];

  if (onExit) onExit();
}

export function handleKey(e) {
  if (!active) return false;

  if (e.key === 'Escape') {
    e.preventDefault();
    if (killConfirmPid !== null) {
      killConfirmPid = null;
      render();
    } else {
      exit();
    }
    return true;
  }

  if (e.key === 'Tab' || (e.code === 'Tab' && e.key === 'Unidentified')) {
    e.preventDefault();
    if (e.shiftKey) {
      selectedIndex = (selectedIndex - 1 + COMMANDS.length) % COMMANDS.length;
    } else {
      selectedIndex = (selectedIndex + 1) % COMMANDS.length;
    }
    activeCommandId = COMMANDS[selectedIndex].id;
    killConfirmPid = null;
    render();
    autoRun();
    if (onCommandChange) onCommandChange();
    return true;
  }

  // Ctrl+1..5 jump to command
  if (e.ctrlKey && !e.shiftKey && e.key >= '1' && e.key <= String(COMMANDS.length)) {
    e.preventDefault();
    const idx = parseInt(e.key) - 1;
    if (idx < COMMANDS.length) {
      selectedIndex = idx;
      activeCommandId = COMMANDS[idx].id;
      killConfirmPid = null;
      render();
      autoRun();
      if (onCommandChange) onCommandChange();
    }
    return true;
  }

  // Kill-specific keys
  if (activeCommandId === 'kill') {
    if (killConfirmPid !== null) {
      if (e.key === 'y' || e.key === 'Y') {
        e.preventDefault();
        if (onExecute) onExecute('kill-execute', String(killConfirmPid));
        killConfirmPid = null;
        return true;
      }
      if (e.key === 'n' || e.key === 'N') {
        e.preventDefault();
        killConfirmPid = null;
        render();
        renderProcessList();
        return true;
      }
      return false;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (filteredProcesses.length > 0) {
        processSelectedIndex = Math.min(processSelectedIndex + 1, filteredProcesses.length - 1);
        renderProcessList();
      }
      return true;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (filteredProcesses.length > 0) {
        processSelectedIndex = Math.max(processSelectedIndex - 1, 0);
        renderProcessList();
      }
      return true;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      if (filteredProcesses.length > 0 && processSelectedIndex < filteredProcesses.length) {
        killConfirmPid = filteredProcesses[processSelectedIndex].pid;
        render();
        renderProcessList();
      }
      return true;
    }
    return false;
  }

  // Pomo keys
  if (activeCommandId === 'pomo') {
    if (e.key === 'Enter') {
      e.preventDefault();
      togglePomo();
      return true;
    }
    if (e.key === 'r' || e.key === 'R') {
      e.preventDefault();
      resetPomo();
      return true;
    }
    return true;
  }

  if (e.key === 'Enter') {
    e.preventDefault();
    const input = cmdInput ? cmdInput.value.trim() : '';
    if (activeCommandId === 'sys') {
      if (onExecute) onExecute('sys-load');
    } else {
      if (onExecute) onExecute(activeCommandId, input);
    }
    return true;
  }

  return false;
}

export function showFeedback(text, isError = false) {
  const feedback = container.querySelector('.cmd-feedback');
  if (feedback) {
    feedback.textContent = text;
    feedback.className = `cmd-feedback ${isError ? 'cmd-feedback-error' : ''}`;
  }
}

export function handleInput() {
  if (!active || !cmdInput) return;
  if (activeCommandId === 'calc') {
    if (cmdInput.value.trim()) {
      if (onExecute) onExecute('calc-preview', cmdInput.value.trim());
    } else {
      showFeedback(getDefaultText('calc'));
    }
  }
  if (activeCommandId === 'kill') {
    filterProcesses(cmdInput.value.trim());
    renderProcessList();
  }
}

export function getActiveCommand() {
  return activeCommandId;
}

export function setProcessList(procs) {
  processList = procs || [];
  processSelectedIndex = 0;
  killConfirmPid = null;
  filterProcesses('');
  render();
  renderProcessList();
}

export function setSysInfo(sections) {
  sysInfoSections = sections;
  renderSysInfo();
}

// --- Render ---

function render() {
  container.innerHTML = '';

  // Sidebar
  const sidebar = document.createElement('div');
  sidebar.className = 'cmd-sidebar';

  COMMANDS.forEach((cmd, i) => {
    const row = document.createElement('div');
    row.className = `cmd-row ${i === selectedIndex ? 'cmd-row-active' : ''}`;

    const icon = document.createElement('span');
    icon.className = 'cmd-row-icon';
    icon.textContent = cmd.icon;
    row.appendChild(icon);

    const text = document.createElement('div');
    text.className = 'cmd-row-text';

    const label = document.createElement('div');
    label.className = 'cmd-row-label';
    label.innerHTML = `${cmd.label} <span class="cmd-row-shortcut">(Ctrl+${cmd.shortcut})</span>`;
    text.appendChild(label);

    const detail = document.createElement('div');
    detail.className = 'cmd-row-detail';
    detail.textContent = cmd.detail;
    text.appendChild(detail);

    row.appendChild(text);

    row.addEventListener('click', () => {
      selectedIndex = i;
      activeCommandId = cmd.id;
      killConfirmPid = null;
      render();
      autoRun();
      if (onCommandChange) onCommandChange();
    });

    sidebar.appendChild(row);
  });

  // Divider
  const divider = document.createElement('div');
  divider.className = 'cmd-divider';

  // Main content
  const main = document.createElement('div');
  main.className = 'cmd-main';

  // Input bar or header bar
  if (ACCEPTS_INPUT.has(activeCommandId)) {
    const inputBar = document.createElement('div');
    inputBar.className = 'cmd-input-bar';

    const barIcon = document.createElement('span');
    barIcon.className = 'cmd-input-bar-icon';
    barIcon.textContent = ICON_MAP[activeCommandId] || '>';
    inputBar.appendChild(barIcon);

    const input = document.createElement('input');
    input.type = 'text';
    input.placeholder = getPlaceholder(activeCommandId);
    input.spellcheck = false;
    input.autocomplete = 'off';
    inputBar.appendChild(input);

    const pill = document.createElement('span');
    pill.className = 'cmd-pill';
    pill.textContent = `/${activeCommandId}`;
    inputBar.appendChild(pill);

    main.appendChild(inputBar);

    cmdInput = input;
    input.addEventListener('input', () => handleInput());
    requestAnimationFrame(() => input.focus());
  } else if (activeCommandId === 'pomo') {
    const headerBar = document.createElement('div');
    headerBar.className = 'cmd-header-bar';

    const barIcon = document.createElement('span');
    barIcon.className = 'cmd-input-bar-icon';
    barIcon.textContent = ICON_MAP.pomo;
    headerBar.appendChild(barIcon);

    const subtitle = document.createElement('span');
    subtitle.className = 'cmd-subtitle';
    subtitle.textContent = 'Enter start/pause \u2022 R reset';
    headerBar.appendChild(subtitle);

    const pill = document.createElement('span');
    pill.className = 'cmd-pill';
    pill.textContent = '/pomo';
    headerBar.appendChild(pill);

    main.appendChild(headerBar);
    cmdInput = null;
  } else {
    // sys — read-only header
    const headerBar = document.createElement('div');
    headerBar.className = 'cmd-header-bar';

    const barIcon = document.createElement('span');
    barIcon.className = 'cmd-input-bar-icon';
    barIcon.textContent = ICON_MAP.sys;
    headerBar.appendChild(barIcon);

    const subtitle = document.createElement('span');
    subtitle.className = 'cmd-subtitle';
    subtitle.textContent = 'Read-only command';
    headerBar.appendChild(subtitle);

    const pill = document.createElement('span');
    pill.className = 'cmd-pill';
    pill.textContent = '/sys';
    headerBar.appendChild(pill);

    main.appendChild(headerBar);
    cmdInput = null;
  }

  // Content area
  const content = document.createElement('div');
  content.className = 'cmd-content';

  if (activeCommandId === 'pomo') {
    renderPomoContent(content);
  } else {
    const feedback = document.createElement('div');
    feedback.className = 'cmd-feedback';
    if (activeCommandId !== 'kill' && activeCommandId !== 'sys') {
      feedback.textContent = getDefaultText(activeCommandId);
    }
    content.appendChild(feedback);
  }

  main.appendChild(content);

  // Kill confirm bar — pinned below the app list
  if (activeCommandId === 'kill' && killConfirmPid !== null) {
    const proc = filteredProcesses.find((p) => p.pid === killConfirmPid);
    const confirmBar = document.createElement('div');
    confirmBar.className = 'cmd-confirm-bar';

    const confirmLeft = document.createElement('div');
    confirmLeft.className = 'cmd-confirm-left';

    const confirmIcon = document.createElement('img');
    confirmIcon.className = 'cmd-proc-icon';
    confirmIcon.width = 28;
    confirmIcon.height = 28;
    confirmIcon.alt = '';
    if (getIconFn && proc?.desktop_id) {
      getIconFn('app', proc.exec || '', proc.desktop_id).then((result) => {
        if (result?.data_url) confirmIcon.src = result.data_url;
        else confirmIcon.style.display = 'none';
      });
    } else {
      confirmIcon.style.display = 'none';
    }
    confirmLeft.appendChild(confirmIcon);

    const confirmText = document.createElement('div');
    confirmText.className = 'cmd-confirm-text';
    const confirmTitle = document.createElement('div');
    confirmTitle.className = 'cmd-confirm-title';
    confirmTitle.textContent = `Kill ${proc ? proc.name : ''}?`;
    confirmText.appendChild(confirmTitle);
    const confirmPid = document.createElement('div');
    confirmPid.className = 'cmd-confirm-pid';
    confirmPid.textContent = `PID: ${killConfirmPid}`;
    confirmText.appendChild(confirmPid);
    confirmLeft.appendChild(confirmText);

    const confirmRight = document.createElement('div');
    confirmRight.className = 'cmd-confirm-right';
    confirmRight.innerHTML = '<span class="cmd-confirm-yes">Y / Yes</span><span class="cmd-confirm-no">N / No</span>';

    confirmBar.appendChild(confirmLeft);
    confirmBar.appendChild(confirmRight);
    main.appendChild(confirmBar);
  }

  container.appendChild(sidebar);
  container.appendChild(divider);
  container.appendChild(main);

  // Hide main search bar in command mode
  mainSearchInput.parentElement.style.display = 'none';
}

function autoRun() {
  if (activeCommandId === 'kill') {
    if (onExecute) onExecute('kill-load');
  }
  if (activeCommandId === 'sys') {
    if (onExecute) onExecute('sys-load');
  }
}

function renderProcessList() {
  const content = container.querySelector('.cmd-content');
  if (!content) return;

  // Remove old process list
  const old = content.querySelector('.cmd-process-list');
  if (old) old.remove();

  const feedback = content.querySelector('.cmd-feedback');
  if (feedback) feedback.textContent = '';

  if (filteredProcesses.length === 0) {
    if (feedback) feedback.textContent = processList.length > 0 ? 'No matching processes' : 'Loading...';
    return;
  }

  const list = document.createElement('div');
  list.className = 'cmd-process-list';

  filteredProcesses.forEach((proc, i) => {
    const row = document.createElement('div');
    row.className = `cmd-proc-row ${i === processSelectedIndex ? 'cmd-proc-row-active' : ''}`;

    // App icon
    const iconEl = document.createElement('img');
    iconEl.className = 'cmd-proc-icon';
    iconEl.width = 22;
    iconEl.height = 22;
    iconEl.alt = '';
    row.appendChild(iconEl);

    // Load icon async
    if (getIconFn && proc.desktop_id) {
      getIconFn('app', proc.exec || '', proc.desktop_id).then((result) => {
        if (result?.data_url) iconEl.src = result.data_url;
        else iconEl.style.display = 'none';
      });
    } else {
      iconEl.style.display = 'none';
    }

    const name = document.createElement('span');
    name.className = 'cmd-proc-name';
    name.textContent = proc.name;
    row.appendChild(name);

    const pid = document.createElement('span');
    pid.className = 'cmd-proc-pid';
    if (i === processSelectedIndex) {
      pid.innerHTML = `PID: ${proc.pid} <span class="cmd-proc-enter">\u2192 Enter</span>`;
    } else {
      pid.textContent = `PID: ${proc.pid}`;
    }
    row.appendChild(pid);

    row.addEventListener('click', () => {
      processSelectedIndex = i;
      killConfirmPid = proc.pid;
      render();
      renderProcessList();
    });

    list.appendChild(row);
  });

  content.appendChild(list);

  // Scroll active into view
  const activeRow = list.querySelector('.cmd-proc-row-active');
  if (activeRow) activeRow.scrollIntoView({ block: 'nearest' });
}

function renderSysInfo() {
  const content = container.querySelector('.cmd-content');
  if (!content) return;

  const feedback = content.querySelector('.cmd-feedback');
  if (feedback) feedback.textContent = '';

  // Remove old sys table
  const old = content.querySelector('.cmd-sys-table');
  if (old) old.remove();

  if (!sysInfoSections || sysInfoSections.length === 0) {
    if (feedback) feedback.textContent = 'No data';
    return;
  }

  const table = document.createElement('div');
  table.className = 'cmd-sys-table';

  sysInfoSections.forEach((section, si) => {
    if (si > 0) {
      const spacer = document.createElement('div');
      spacer.className = 'cmd-sys-spacer';
      table.appendChild(spacer);
    }

    section.forEach((entry) => {
      const row = document.createElement('div');
      row.className = 'cmd-sys-row';

      const label = document.createElement('span');
      label.className = 'cmd-sys-label';
      label.textContent = entry.label;
      row.appendChild(label);

      const value = document.createElement('span');
      value.className = 'cmd-sys-value';
      value.textContent = entry.value;
      row.appendChild(value);

      table.appendChild(row);
    });
  });

  content.appendChild(table);
}

// --- Pomo ---

function togglePomo() {
  if (pomoState === 'idle') {
    pomoState = 'work';
    pomoRemaining = POMO_WORK;
    startPomoTimer();
  } else if (pomoInterval) {
    // Pause
    clearInterval(pomoInterval);
    pomoInterval = null;
    renderPomoDisplay();
  } else {
    // Resume
    startPomoTimer();
  }
  renderPomoDisplay();
}

function resetPomo() {
  if (pomoInterval) clearInterval(pomoInterval);
  pomoInterval = null;
  pomoState = 'idle';
  pomoRemaining = 0;
  renderPomoDisplay();
}

function startPomoTimer() {
  if (pomoInterval) clearInterval(pomoInterval);
  pomoInterval = setInterval(() => {
    pomoRemaining--;
    if (pomoRemaining <= 0) {
      clearInterval(pomoInterval);
      pomoInterval = null;
      if (pomoState === 'work') {
        pomoState = 'break';
        pomoRemaining = POMO_BREAK;
        startPomoTimer();
      } else {
        pomoState = 'idle';
        pomoRemaining = 0;
      }
    }
    renderPomoDisplay();
  }, 1000);
}

function renderPomoContent(content) {
  const display = document.createElement('div');
  display.className = 'cmd-pomo-display';

  const time = document.createElement('div');
  time.className = 'cmd-pomo-time';
  time.textContent = formatPomoTime();
  display.appendChild(time);

  const status = document.createElement('div');
  status.className = 'cmd-pomo-status';
  status.textContent = getPomoStatusText();
  display.appendChild(status);

  content.appendChild(display);
}

function renderPomoDisplay() {
  const time = container?.querySelector('.cmd-pomo-time');
  const status = container?.querySelector('.cmd-pomo-status');
  if (time) time.textContent = formatPomoTime();
  if (status) status.textContent = getPomoStatusText();
}

function formatPomoTime() {
  if (pomoState === 'idle') return '25:00';
  const m = Math.floor(pomoRemaining / 60);
  const s = pomoRemaining % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

function getPomoStatusText() {
  if (pomoState === 'idle') return 'Press Enter to start focus session';
  if (pomoState === 'work') return pomoInterval ? 'Focusing...' : 'Paused';
  if (pomoState === 'break') return pomoInterval ? 'Break time' : 'Paused (break)';
  return '';
}

// --- Helpers ---

function filterProcesses(query) {
  if (!query) {
    filteredProcesses = [...processList];
  } else if (query.startsWith(':')) {
    // Port search — not implemented yet, show all
    filteredProcesses = [...processList];
  } else {
    const q = query.toLowerCase();
    filteredProcesses = processList.filter((p) => p.name.toLowerCase().includes(q));
  }
  processSelectedIndex = Math.min(processSelectedIndex, Math.max(0, filteredProcesses.length - 1));
}

function getPlaceholder(cmdId) {
  switch (cmdId) {
    case 'calc': return 'Type math expression';
    case 'shell': return 'Type shell command...';
    case 'kill': return 'Type app name, or :3000';
    default: return 'Type command...';
  }
}

function getDefaultText(cmdId) {
  switch (cmdId) {
    case 'calc': return 'Selected /calc';
    case 'shell': return 'Selected /shell';
    default: return '';
  }
}
