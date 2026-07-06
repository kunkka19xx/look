const MAX_PORT = 65535;
const PORT_DEBOUNCE_MS = 200;

let panel = null;
let input = null;
let feedback = null;
let listEl = null;
let confirmBar = null;
let confirmIcon = null;
let confirmTitle = null;
let confirmPidEl = null;

let onExecute = null;
let getIconFn = null;

let baseProcessList = [];
let processList = [];
let filteredProcesses = [];
let selectedIndex = 0;
let confirmPid = null;
let portDebounce = null;

export function init(executeFn, iconFn) {
  onExecute = executeFn;
  getIconFn = iconFn;
  panel = document.getElementById('cmd-panel-kill');
  input = document.getElementById('cmd-kill-input');
  feedback = document.getElementById('cmd-kill-feedback');
  listEl = document.getElementById('cmd-kill-list');
  confirmBar = document.getElementById('cmd-kill-confirm');
  confirmIcon = document.getElementById('cmd-kill-confirm-icon');
  confirmTitle = document.getElementById('cmd-kill-confirm-title');
  confirmPidEl = document.getElementById('cmd-kill-confirm-pid');

  input.addEventListener('input', () => {
    filterProcesses(input.value.trim());
    renderList();
  });
}

export function enter() {
  panel.hidden = false;
  input.value = '';
  confirmPid = null;
  updateConfirmBar();
  requestAnimationFrame(() => input.focus());
  if (onExecute) onExecute('kill-load');
}

export function exit() {
  panel.hidden = true;
  confirmPid = null;
}

export function handleKey(e) {
  // Escape dismisses confirm
  if (e.key === 'Escape' && confirmPid !== null) {
    confirmPid = null;
    updateConfirmBar();
    updateSelection();
    return true;
  }

  // Confirm state - consume all keys, only act on Y/N
  if (confirmPid !== null) {
    e.preventDefault();
    if (e.key === 'y' || e.key === 'Y') {
      if (onExecute) onExecute('kill-execute', String(confirmPid));
      confirmPid = null;
      updateConfirmBar();
    } else if (e.key === 'n' || e.key === 'N') {
      confirmPid = null;
      updateConfirmBar();
      updateSelection();
    }
    return true;
  }

  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (filteredProcesses.length > 0) {
      selectedIndex = Math.min(selectedIndex + 1, filteredProcesses.length - 1);
      updateSelection();
    }
    return true;
  }
  if (e.key === 'ArrowUp') {
    e.preventDefault();
    if (filteredProcesses.length > 0) {
      selectedIndex = Math.max(selectedIndex - 1, 0);
      updateSelection();
    }
    return true;
  }
  if (e.key === 'Enter') {
    e.preventDefault();
    if (filteredProcesses.length > 0 && selectedIndex < filteredProcesses.length) {
      confirmPid = filteredProcesses[selectedIndex].pid;
      updateConfirmBar();
      updateSelection();
    }
    return true;
  }
  return false;
}

export function setProcessList(procs, isPortResult = false) {
  const savedValue = input ? input.value : '';
  if (isPortResult) {
    processList = procs || [];
    filteredProcesses = [...processList];
  } else {
    baseProcessList = procs || [];
    processList = [...baseProcessList];
    filterProcesses(savedValue);
  }
  selectedIndex = 0;
  confirmPid = null;
  renderList();
  updateConfirmBar();
}

export function showFeedback(text, isError = false) {
  feedback.textContent = text;
  feedback.className = `cmd-feedback ${isError ? 'cmd-feedback-error' : ''}`;
}

// --- Internal ---

function filterProcesses(query) {
  if (!query) {
    processList = [...baseProcessList];
    filteredProcesses = [...processList];
  } else if (query.startsWith(':')) {
    filteredProcesses = [];
    selectedIndex = 0;
    const port = parseInt(query.slice(1));
    if (port > 0 && port <= MAX_PORT) {
      clearTimeout(portDebounce);
      portDebounce = setTimeout(() => {
        if (onExecute) onExecute('kill-port', String(port));
      }, PORT_DEBOUNCE_MS);
    }
    return;
  } else {
    const q = query.toLowerCase();
    filteredProcesses = baseProcessList.filter((p) => p.name.toLowerCase().includes(q));
  }
  selectedIndex = Math.min(selectedIndex, Math.max(0, filteredProcesses.length - 1));
}

function renderList() {
  listEl.innerHTML = '';
  feedback.textContent = '';

  if (filteredProcesses.length === 0) {
    const query = input ? input.value.trim() : '';
    if (query.startsWith(':')) {
      const port = query.slice(1);
      feedback.textContent = processList.length === 0 && port ? `No process on port ${port}` : 'Searching port...';
    } else {
      feedback.textContent = processList.length > 0 ? 'No matching processes' : 'Loading...';
    }
    return;
  }

  filteredProcesses.forEach((proc, i) => {
    const row = document.createElement('div');
    row.className = `cmd-proc-row ${i === selectedIndex ? 'cmd-proc-row-active' : ''}`;

    const iconEl = document.createElement('img');
    iconEl.className = 'cmd-proc-icon';
    iconEl.width = 22;
    iconEl.height = 22;
    iconEl.alt = '';
    row.appendChild(iconEl);

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
    if (i === selectedIndex) {
      pid.innerHTML = `PID: ${proc.pid} <span class="cmd-proc-enter">\u2192 Enter</span>`;
    } else {
      pid.textContent = `PID: ${proc.pid}`;
    }
    row.appendChild(pid);

    row.addEventListener('click', () => {
      selectedIndex = i;
      confirmPid = proc.pid;
      updateConfirmBar();
      updateSelection();
    });

    listEl.appendChild(row);
  });
}

function updateSelection() {
  const rows = listEl.children;
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    const isActive = i === selectedIndex;
    row.classList.toggle('cmd-proc-row-active', isActive);

    const pidEl = row.querySelector('.cmd-proc-pid');
    if (pidEl) {
      const proc = filteredProcesses[i];
      if (isActive) {
        pidEl.innerHTML = `PID: ${proc.pid} <span class="cmd-proc-enter">\u2192 Enter</span>`;
      } else {
        pidEl.textContent = `PID: ${proc.pid}`;
      }
    }
  }

  const activeRow = listEl.querySelector('.cmd-proc-row-active');
  if (activeRow) activeRow.scrollIntoView({ block: 'nearest' });
}

function updateConfirmBar() {
  if (confirmPid === null) {
    confirmBar.hidden = true;
    return;
  }
  const proc = filteredProcesses.find((p) => p.pid === confirmPid);
  confirmBar.hidden = false;
  confirmTitle.textContent = `Kill ${proc ? proc.name : ''}?`;
  confirmPidEl.textContent = `PID: ${confirmPid}`;

  confirmIcon.style.display = 'none';
  if (getIconFn && proc?.desktop_id) {
    getIconFn('app', proc.exec || '', proc.desktop_id).then((result) => {
      if (result?.data_url) {
        confirmIcon.src = result.data_url;
        confirmIcon.style.display = '';
      }
    });
  }
}
