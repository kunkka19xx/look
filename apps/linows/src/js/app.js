import * as results from './components/results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './components/preview.js';
import * as picked from './components/picked.js';
import * as banner from './components/banner.js';
import * as commands from './screens/commands/index.js';
import * as settings from './screens/settings.js';
import * as translatePanel from './components/translate.js';
import * as platform from './platform.js';
import { load } from './html-loader.js';
import {
  onWindowShown, onIndexReady, requestIndexRefresh, getHomeDir, getQuickFolders, copyFilesToClipboard,
  evalCalc, runShellCommand, getSystemInfo,
  listProcesses, listProcessesOnPort, killProcess, getIcon,
  copyToClipboard, deleteClipboardEntry, isDevBuild,
} from './ipc.js';

// Item count and structure mirror the macOS app's `LauncherView.hintItems`
// (apps/macos/.../LauncherView.swift:302) so both platforms surface the same
// shortcuts in the same modes. Style stays per-platform: linows uses the
// colon + bold-bullet format, macOS keeps its space-separated form.
const HINT_MAIN = 'Enter: Open \u2022 Ctrl+F: Reveal \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';
const HINT_TRANSLATE = 'Enter: Translate \u2022 Copy per result \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';
const HINT_CLIPBOARD = 'Enter: Copy clip \u2022 Delete: Remove clip \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';

// Hint constants are static, authored in code \u2014 safe to set as innerHTML so
// each bullet renders through `.hint-sep` (accent color, bold) for clearer
// visual separation between key/action pairs.
function setHint(el, text) {
  el.innerHTML = text.split(' \u2022 ').join(' <span class="hint-sep">\u2022</span> ');
}
const BANNER_DURATION_SHORT = 1.0;
const BANNER_DURATION_MEDIUM = 1.2;
const BANNER_DURATION_LONG = 1.5;
const KILL_FEEDBACK_DELAY_MS = 300;

document.addEventListener('DOMContentLoaded', async () => {
  const app = document.getElementById('app');

  // Detect platform early
  await platform.init();

  // Load screen templates
  await load('html/screens/search.html', app);
  await load('html/screens/commands/index.html', app);
  await load('html/screens/settings.html', app);
  await load('html/screens/help.html', app);

  // Hint bar — always at bottom, shared by all screens
  app.insertAdjacentHTML('beforeend',
    `<div class="hint-bar" id="hint-bar"><span></span><span class="hint-bar-copy">\u00A9 2026 by <a class="hint-bar-link" href="#">Kunkka</a></span></div>`);

  // Load command panels into cmd-main
  const cmdMain = document.getElementById('cmd-main');
  await Promise.all([
    load('html/screens/commands/calc.html', cmdMain),
    load('html/screens/commands/pomo.html', cmdMain),
    load('html/screens/commands/kill.html', cmdMain),
    load('html/screens/commands/shell.html', cmdMain),
    load('html/screens/commands/sys.html', cmdMain),
  ]);

  // DOM refs
  const queryInput = document.getElementById('query');
  const resultsList = document.getElementById('results-list');
  const previewPanel = document.getElementById('preview-panel');
  const hintBar = document.getElementById('hint-bar');
  const hintMessage = hintBar.querySelector('span');
  const contentArea = document.getElementById('search-content');
  setHint(hintMessage, HINT_MAIN);

  hintBar.querySelector('.hint-bar-link').addEventListener('click', (e) => {
    e.preventDefault();
    import('./ipc.js').then(({ openPath }) => {
      openPath('https://github.com/kunkka19xx', 'browser', '');
    });
  });

  // Initialize modules
  results.init(resultsList);
  keyboard.init(queryInput);
  preview.init(previewPanel);
  banner.init(document.getElementById('banner'));
  picked.init(previewPanel, {
    onRemoveItem: (key) => results.removePick(key),
    onClearAll: () => results.clearPicks(),
  });
  commands.init(contentArea, queryInput, {
    onExitMode: exitCommandMode,
    onExecuteCommand: executeCommand,
    onGetIcon: getIcon,
  });
  translatePanel.init(contentArea);
  settings.init(() => {
    queryInput.value = '';
    search.handleQueryInput('');
    queryInput.focus();
    setHint(hintMessage, HINT_MAIN);
  });
  settings.restoreOnStartup();

  // Show DEV badge when running in dev mode (cargo tauri dev)
  isDevBuild().then((isDev) => {
    if (isDev) {
      const badge = document.createElement('span');
      badge.className = 'dev-badge';
      badge.textContent = 'DEV';
      document.getElementById('search-bar').appendChild(badge);
    }
  });

  // Expose command mode toggle for keyboard.js
  keyboard.setCommandMode(commands);

  // Update right panel when selection changes
  results.setOnSelectionChange((item) => {
    if (!results.hasPickedItems()) {
      previewPanel.hidden = false;
      preview.update(item);
    }
  });

  // Wire clipboard delete from preview panel
  preview.setOnClipDelete(() => {
    search.handleQueryInput(queryInput.value);
  });

  // Update right panel when picks change + auto-copy
  results.setOnPickChange((pickedItems) => {
    if (pickedItems.length > 0) {
      preview.clear();
      picked.update(pickedItems);
      const paths = pickedItems
        .filter((i) => i.kind === 'file' || i.kind === 'folder')
        .map((i) => i.path);
      if (paths.length > 0) {
        copyFilesToClipboard(paths)
          .then(() => banner.show(`Picked ${pickedItems.length} item(s)`, 'success', BANNER_DURATION_SHORT))
          .catch(() => banner.show('Pick failed', 'error', BANNER_DURATION_MEDIUM));
      } else {
        banner.show(`Picked ${pickedItems.length} item(s)`, 'success', BANNER_DURATION_SHORT);
      }
    } else {
      picked.update([]);
      preview.update(results.getSelected());
    }
  });

  // Wire search -> results
  search.setOnResults((items, query) => {
    results.render(items);
  });

  // :cmd prefix → jump into command mode (e.g. :calc 2+2, :kill chrome)
  const CMD_PREFIX_MAP = { calc: 'calc', pomo: 'pomo', kill: 'kill', shell: 'shell', sys: 'sys' };

  function tryCommandPrefix(value) {
    if (!value.startsWith(':')) return false;
    const rest = value.slice(1);
    const spaceIdx = rest.indexOf(' ');
    const cmdName = spaceIdx >= 0 ? rest.slice(0, spaceIdx) : rest;
    const cmdId = CMD_PREFIX_MAP[cmdName.toLowerCase()];
    if (!cmdId) return false;
    const input = spaceIdx >= 0 ? rest.slice(spaceIdx + 1) : '';
    commands.enterById(cmdId);
    enterCommandMode();
    // Set the command input if there's text after the command name
    const cmdInput = document.getElementById('cmd-input');
    if (cmdInput && input) {
      cmdInput.value = input;
      cmdInput.dispatchEvent(new Event('input'));
    }
    queryInput.value = '';
    return true;
  }

  // Search on input
  queryInput.addEventListener('input', (e) => {
    const value = e.target.value;
    if (tryCommandPrefix(value)) return;

    search.handleQueryInput(value);
    if (search.isTranslateMode()) {
      setHint(hintMessage, HINT_TRANSLATE);
      resultsList.hidden = true;
      previewPanel.hidden = true;
      if (!translatePanel.isActive()) translatePanel.showPlaceholder();
    } else if (search.isClipboardMode()) {
      setHint(hintMessage, HINT_CLIPBOARD);
      resultsList.hidden = false;
      translatePanel.hide();
    } else {
      setHint(hintMessage, HINT_MAIN);
      resultsList.hidden = false;
      translatePanel.hide();
    }
  });

  // Click on result row -> open
  resultsList.addEventListener('result-activate', () => {
    const item = results.getSelected();
    if (item) {
      import('./ipc.js').then(({ openPath, recordUsage }) => {
        openPath(item.path, item.kind, item.id);
        const actionMap = { app: 'open_app', file: 'open_file', folder: 'open_folder' };
        recordUsage(item.id, actionMap[item.kind] || 'open_file');
      });
    }
  });

  // When window shown via global hotkey, focus input and select all
  onWindowShown(() => {
    queryInput.focus();
    queryInput.select();
    requestIndexRefresh();
  });

  onIndexReady(() => {
    search.handleQueryInput(queryInput.value);
  });

  // Guard: if focus drifts to another element on the main screen,
  // pull it back to the search input.
  document.addEventListener('focusin', (e) => {
    if (e.target !== queryInput
        && !commands.isActive()
        && !settings.isActive()
        && !helpScreen?.contains(e.target)) {
      queryInput.focus();
    }
  });

  // Load home dir + resolved quick-folder paths (Desktop, Documents, …).
  // Quick folders use SHGetKnownFolderPath on Windows to handle OneDrive
  // redirection; on Linux/macOS they're $HOME/<name>.
  Promise.all([getHomeDir(), getQuickFolders()]).then(([home, folders]) => {
    if (home) search.setHomeDir(home);
    search.setQuickFolders(folders || []);
    search.handleQueryInput('');
  });

  // --- Command mode helpers ---

  function enterCommandMode() {
    resultsList.hidden = true;
    previewPanel.hidden = true;
    updateCommandHintBar();
    commands.enter();
    commands.setOnCommandChange(updateCommandHintBar);
  }

  function updateCommandHintBar() {
    const cmd = commands.getActiveCommand();
    if (cmd === 'pomo') {
      setHint(hintMessage,
        'Space: Start/pause \u2022 R: Reset \u2022 P: Music \u2022 Esc: Back \u2022 Tab/Ctrl+1-5: Switch');
    } else if (cmd === 'kill') {
      setHint(hintMessage,
        'Y: Confirm \u2022 N: Cancel \u2022 Tab/Ctrl+1-5: Switch \u2022 Esc: Back');
    } else if (cmd === 'sys') {
      setHint(hintMessage,
        'Esc: Back \u2022 Tab/Ctrl+1-5: Switch \u2022 Ctrl+/: Command mode \u2022 Ctrl+Shift+,: Settings');
    } else if (cmd === 'calc') {
      setHint(hintMessage,
        'Enter: Evaluate \u2022 Tab: Select \u2022 Ctrl+1-5: Switch \u2022 Esc: Back');
    } else if (cmd === 'shell') {
      setHint(hintMessage,
        'Enter: Run \u2022 Tab: Select \u2022 Ctrl+1-5: Switch \u2022 Esc: Back');
    } else {
      setHint(hintMessage,
        'Enter: Run \u2022 Tab: Select \u2022 Ctrl+1-5: Switch \u2022 Esc: Back');
    }
  }

  function exitCommandMode() {
    queryInput.parentElement.style.display = '';
    resultsList.hidden = false;
    previewPanel.hidden = false;
    translatePanel.hide();
    setHint(hintMessage, HINT_MAIN);
    queryInput.value = '';
    search.handleQueryInput('');
    queryInput.focus();
  }

  async function executeCommand(cmdId, input) {
    if (cmdId === 'calc-preview') {
      try {
        const result = await evalCalc(input);
        commands.showFeedback(result);
      } catch {
        // Don't show errors during live preview
      }
      return;
    }

    if (cmdId === 'kill-load') {
      try {
        const procs = await listProcesses();
        commands.setProcessList(procs);
      } catch (err) {
        commands.showFeedback(err || 'Failed to list processes', true);
      }
      return;
    }

    if (cmdId === 'kill-port') {
      const port = parseInt(input);
      if (!port) return;
      try {
        const procs = await listProcessesOnPort(port);
        commands.setProcessList(procs, true);
      } catch (err) {
        commands.showFeedback(err || 'Failed to query port', true);
      }
      return;
    }

    if (cmdId === 'kill-execute') {
      const pid = parseInt(input);
      if (!pid) return;
      try {
        const msg = await killProcess(pid);
        banner.show(msg, 'success', BANNER_DURATION_MEDIUM);
        await new Promise((r) => setTimeout(r, KILL_FEEDBACK_DELAY_MS));
        const procs = await listProcesses();
        commands.setProcessList(procs);
      } catch (err) {
        banner.show(err || 'Kill failed', 'error', BANNER_DURATION_LONG);
      }
      return;
    }

    if (cmdId === 'sys-load') {
      try {
        const sections = await getSystemInfo();
        commands.setSysInfo(sections);
      } catch (err) {
        commands.showFeedback(err || 'Failed to get system info', true);
      }
      return;
    }

    switch (cmdId) {
      case 'calc':
        if (!input) return;
        try {
          const result = await evalCalc(input);
          commands.showFeedback(result);
          await navigator.clipboard.writeText(result);
          banner.show('Result copied', 'success', BANNER_DURATION_SHORT);
        } catch (err) {
          commands.showFeedback(err || 'Invalid expression', true);
        }
        break;

      case 'shell':
        if (!input) return;
        commands.showFeedback('Running...');
        try {
          const output = await runShellCommand(input);
          commands.showFeedback(output);
        } catch (err) {
          commands.showFeedback(err || 'Command failed', true);
        }
        break;

      case 'sys':
        executeCommand('sys-load');
        break;
    }
  }

  // Expose enterCommandMode and settings for keyboard
  keyboard.setEnterCommandMode(enterCommandMode);
  keyboard.setSettingsMode(settings, contentArea, queryInput.parentElement);
});
