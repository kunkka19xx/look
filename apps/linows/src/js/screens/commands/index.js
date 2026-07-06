import * as calc from './calc.js';
import * as pomo from './pomo.js';
import * as todo from './todo.js';
import * as kill from './kill.js';
import * as shell from './shell.js';
import * as sys from './sys.js';
import { calculator, timer, listChecks, xCircle, terminal, info } from '../../icons.js';

const COMMANDS = [
  { id: 'calc', label: '/calc', shortcut: '1', detail: 'Evaluate math...', icon: calculator, module: calc },
  { id: 'pomo', label: '/pomo', shortcut: '2', detail: 'Pomodoro focus...', icon: timer, module: pomo },
  { id: 'todo', label: '/todo', shortcut: '3', detail: 'Daily tasks &...', icon: listChecks, module: todo },
  { id: 'kill', label: '/kill', shortcut: '4', detail: 'Force kill app...', icon: xCircle, module: kill },
  { id: 'shell', label: '/shell', shortcut: '5', detail: 'Run a shell co...', icon: terminal, module: shell },
  { id: 'sys', label: '/sys', shortcut: '6', detail: 'Show system in...', icon: info, module: sys },
];

let screen = null;
let sidebar = null;
let contentArea = null;
let mainSearchInput = null;

let active = false;
let selectedIndex = 0;
let activeCommandId = 'calc';
let onExit = null;
let onCommandChange = null;

export function init(contentAreaEl, inputEl, { onExitMode, onExecuteCommand, onGetIcon }) {
  contentArea = contentAreaEl;
  mainSearchInput = inputEl;
  onExit = onExitMode;

  screen = document.getElementById('commands-screen');
  sidebar = document.getElementById('cmd-sidebar');

  // Init each command module
  calc.init(onExecuteCommand);
  pomo.init();
  todo.init();
  kill.init(onExecuteCommand, onGetIcon);
  shell.init(onExecuteCommand);
  sys.init(onExecuteCommand);

  // Set header bar icons
  document.getElementById('cmd-calc-header-icon').innerHTML = calculator;
  document.getElementById('cmd-kill-header-icon').innerHTML = xCircle;
  document.getElementById('cmd-shell-header-icon').innerHTML = terminal;
  document.getElementById('cmd-sys-header-icon').innerHTML = info;

  buildSidebar();
}

export function setOnCommandChange(fn) {
  onCommandChange = fn;
}

export function isActive() {
  return active;
}

export function enterById(cmdId) {
  const idx = COMMANDS.findIndex((c) => c.id === cmdId);
  if (idx < 0) return false;
  activeCommandId = cmdId;
  return true;
}

export function enter() {
  active = true;
  selectedIndex = COMMANDS.findIndex((c) => c.id === activeCommandId);
  if (selectedIndex < 0) selectedIndex = 0;

  contentArea.style.display = 'none';
  screen.style.display = '';
  mainSearchInput.parentElement.style.display = 'none';

  updateSidebar();
  currentModule().enter();
}

export function exit() {
  active = false;
  currentModule().exit();

  screen.style.display = 'none';
  contentArea.style.display = '';
  mainSearchInput.parentElement.style.display = '';

  if (onExit) onExit();
}

export function handleKey(e) {
  if (!active) return false;

  // Let the active command handle Escape first (e.g. dismiss confirm)
  if (e.key === 'Escape') {
    e.preventDefault();
    const handled = currentModule().handleKey(e);
    if (!handled) exit();
    return true;
  }

  if (e.key === 'Tab' || (e.code === 'Tab' && e.key === 'Unidentified')) {
    e.preventDefault();
    switchCommand(e.shiftKey ? -1 : 1);
    return true;
  }

  // Ctrl+1..6 jump to command
  if (e.ctrlKey && !e.shiftKey && e.key >= '1' && e.key <= String(COMMANDS.length)) {
    e.preventDefault();
    const idx = parseInt(e.key) - 1;
    if (idx < COMMANDS.length && idx !== selectedIndex) {
      switchTo(idx);
    }
    return true;
  }

  // Delegate to active command module
  return currentModule().handleKey(e);
}

export function getActiveCommand() {
  return activeCommandId;
}

// Delegate methods for app.js
export function showFeedback(text, isError = false) {
  const mod = currentModule();
  if (mod.showFeedback) mod.showFeedback(text, isError);
}

export function setProcessList(procs, isPortResult = false) {
  kill.setProcessList(procs, isPortResult);
}

export function setSysInfo(sections) {
  sys.setSysInfo(sections);
}

// --- Internal ---

function currentModule() {
  return COMMANDS[selectedIndex].module;
}

function switchCommand(dir) {
  currentModule().exit();
  selectedIndex = (selectedIndex + dir + COMMANDS.length) % COMMANDS.length;
  activeCommandId = COMMANDS[selectedIndex].id;
  updateSidebar();
  currentModule().enter();
  if (onCommandChange) onCommandChange();
}

function switchTo(idx) {
  currentModule().exit();
  selectedIndex = idx;
  activeCommandId = COMMANDS[idx].id;
  updateSidebar();
  currentModule().enter();
  if (onCommandChange) onCommandChange();
}

function buildSidebar() {
  sidebar.innerHTML = '';
  COMMANDS.forEach((cmd, i) => {
    const row = document.createElement('div');
    row.className = 'cmd-row';
    row.innerHTML = `
      <span class="cmd-row-icon">${cmd.icon}</span>
      <div class="cmd-row-text">
        <div class="cmd-row-label">${cmd.label} <span class="cmd-row-shortcut">(Ctrl+${cmd.shortcut})</span></div>
        <div class="cmd-row-detail">${cmd.detail}</div>
      </div>`;
    row.addEventListener('click', () => {
      if (i !== selectedIndex) switchTo(i);
    });
    sidebar.appendChild(row);
  });
}

function updateSidebar() {
  const rows = sidebar.children;
  for (let i = 0; i < rows.length; i++) {
    rows[i].classList.toggle('cmd-row-active', i === selectedIndex);
  }
}
