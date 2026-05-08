import * as results from './components/results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './components/preview.js';
import * as picked from './components/picked.js';
import * as banner from './components/banner.js';
import * as commands from './screens/commands/index.js';
import { load } from './html-loader.js';
import {
  onWindowShown, getHomeDir, copyFilesToClipboard,
  evalCalc, runShellCommand, getSystemInfo,
  listProcesses, listProcessesOnPort, killProcess, getIcon,
} from './ipc.js';

document.addEventListener('DOMContentLoaded', async () => {
  const app = document.getElementById('app');

  // Load screen templates
  await load('html/screens/search.html', app);
  await load('html/screens/commands/index.html', app);

  // Hint bar — always at bottom, shared by all screens
  app.insertAdjacentHTML('beforeend',
    '<div class="hint-bar" id="hint-bar"><span>Enter open \u2022 Ctrl+Enter search web \u2022 Ctrl+P pick \u2022 Ctrl+C copy \u2022 Ctrl+F reveal \u2022 Esc hide</span></div>');

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
  const contentArea = document.getElementById('search-content');

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

  // Expose command mode toggle for keyboard.js
  keyboard.setCommandMode(commands);

  // Update right panel when selection changes
  results.setOnSelectionChange((item) => {
    if (!results.hasPickedItems()) {
      preview.update(item);
    }
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
          .then(() => banner.show(`Picked ${pickedItems.length} item(s)`, 'success', 1.0))
          .catch(() => banner.show('Pick failed', 'error', 1.2));
      } else {
        banner.show(`Picked ${pickedItems.length} item(s)`, 'success', 1.0);
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

  // Search on input
  queryInput.addEventListener('input', (e) => {
    search.handleQueryInput(e.target.value);
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
  });

  // Load home dir for quick folders, then initial search
  getHomeDir().then((home) => {
    if (home) search.setHomeDir(home);
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
    const h = hintBar.querySelector('span');
    if (cmd === 'pomo') {
      h.textContent =
        'Space start/pause \u2022 R reset \u2022 P music \u2022 Esc back \u2022 Tab/Ctrl+1-5 switch';
    } else if (cmd === 'kill') {
      h.textContent =
        'Y confirm \u2022 N cancel \u2022 Tab/Ctrl+1-5 switch \u2022 Esc back';
    } else if (cmd === 'sys') {
      h.textContent =
        'Esc back \u2022 Tab/Ctrl+1-5 switch';
    } else if (cmd === 'calc') {
      h.textContent =
        'Enter evaluate \u2022 Tab/Ctrl+1-5 switch \u2022 Esc back';
    } else if (cmd === 'shell') {
      h.textContent =
        'Enter run \u2022 Tab/Ctrl+1-5 switch \u2022 Esc back';
    } else {
      h.textContent =
        'Tab/Ctrl+1-5 switch \u2022 Esc back';
    }
  }

  function exitCommandMode() {
    queryInput.parentElement.style.display = '';
    resultsList.hidden = false;
    previewPanel.hidden = false;
    hintBar.querySelector('span').textContent =
      'Enter open \u2022 Ctrl+Enter search web \u2022 Ctrl+P pick \u2022 Ctrl+C copy \u2022 Ctrl+F reveal \u2022 Esc hide';
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
        banner.show(msg, 'success', 1.2);
        await new Promise((r) => setTimeout(r, 300));
        const procs = await listProcesses();
        commands.setProcessList(procs);
      } catch (err) {
        banner.show(err || 'Kill failed', 'error', 1.5);
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
          banner.show('Result copied', 'success', 1.0);
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

  // Expose enterCommandMode for keyboard
  keyboard.setEnterCommandMode(enterCommandMode);
});
