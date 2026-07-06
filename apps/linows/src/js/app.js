import * as results from './components/results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './components/preview.js';
import * as picked from './components/picked.js';
import * as banner from './components/banner.js';
import * as confirm from './components/confirm.js';
import * as commands from './screens/commands/index.js';
import * as todoCmd from './screens/commands/todo.js';
import { listChecks } from './icons.js';
import * as settings from './screens/settings.js';
import { mountUpdateWidget } from './screens/update_widget.js';
import * as translatePanel from './components/translate.js';
import * as runningApps from './components/running-apps.js';
import * as platform from './platform.js';
import * as aiAnswer from './components/ai-answer.js';
import { State as AiState } from './components/ai-answer.js';
import * as aiCard from './components/ai-answer-card.js';
import { load } from './html-loader.js';
import {
  onWindowShown, onIndexReady, requestIndexRefresh, getQuickFolders, copyFilesToClipboard,
  evalCalc, runShellCommand, getSystemInfo,
  listProcesses, listProcessesOnPort, killProcess, getIcon,
  copyToClipboard, deleteClipboardEntry, isDevBuild,
  getConfig,
} from './ipc.js';
import {
  prefixFromResultId, commandIdFromResultId, webSuggestionFromResultId, isPrefixedQuery,
} from './catalog.js';

// Item count and structure mirror the macOS app's `LauncherView.hintItems`
// (apps/macos/.../LauncherView.swift:302) so both platforms surface the same
// shortcuts in the same modes. Style stays per-platform: linows uses the
// colon + bold-bullet format, macOS keeps its space-separated form.
const HINT_MAIN = 'Enter: Open \u2022 Ctrl+F: Reveal \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';
const HINT_TRANSLATE = 'Enter: Translate \u2022 Copy per result \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';
const HINT_CLIPBOARD = 'Enter: Copy clip \u2022 Delete: Remove clip \u2022 Ctrl+H: Help \u2022 Ctrl+/: Command mode';
// Discovery-menu hints \u2014 mirror macOS prefixSuggestion / commandSuggestion
// hint bars (LauncherView.swift hintItems).
const HINT_PREFIX_DISCOVERY = 'Enter: Pick prefix \u2022 Up/Down: Move \u2022 Esc: Clear \u2022 Ctrl+H: Help';
const HINT_COMMAND_DISCOVERY = 'Enter: Run command \u2022 Up/Down: Move \u2022 Esc: Clear \u2022 Ctrl+H: Help';

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

// Layout modes applied to #results-area when the AI card is visible.
// Stacked: card capped above results in col 1.
// Two-col: wide card spans both cols + 320 px suggestion column on the right.
const AI_LAYOUT_CLASSES = ['ai-mode-full', 'ai-mode-two-col', 'ai-mode-stacked'];
const AI_LAYOUT_STACKED = 'ai-mode-stacked';
const AI_LAYOUT_TWO_COL = 'ai-mode-two-col';

document.addEventListener('DOMContentLoaded', async () => {
  const app = document.getElementById('app');

  // Detect platform early
  await platform.init();

  // Load screen templates
  await load('html/screens/search.html', app);
  await load('html/screens/commands/index.html', app);
  await load('html/screens/settings.html', app);
  await load('html/screens/help.html', app);

  // About / version + update-status widget, shared between Settings and Help.
  // Settings gets an "About" header label; Help mounts the widget bare so the
  // screen title above already serves as its heading (mirrors macOS layout).
  mountUpdateWidget(document.getElementById('settings-about'), { label: 'About' });
  mountUpdateWidget(document.getElementById('help-update'));

  // Hint bar: always at bottom, shared by all screens
  app.insertAdjacentHTML('beforeend',
    `<div class="hint-bar" id="hint-bar"><span></span><span class="hint-bar-copy">\u00A9 2026 by <a class="hint-bar-link" href="#">Kunkka</a></span></div>`);

  // Load command panels into cmd-main
  const cmdMain = document.getElementById('cmd-main');
  await Promise.all([
    load('html/screens/commands/calc.html', cmdMain),
    load('html/screens/commands/pomo.html', cmdMain),
    load('html/screens/commands/todo.html', cmdMain),
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
  const resultsArea = document.getElementById('results-area');
  const aiCardEl = document.getElementById('ai-answer-card');

  // Todo quick view: when today has tasks, the last main-hint item
  // ("Ctrl+/: Command mode") is swapped for a clickable "Todo X/Y" stat with
  // an "Unfinished today" hover bubble. Mirrors macOS HintBar.TodoQuickView:
  // home screen only, hidden when today is empty.
  let todoQuick = null;

  function renderMainHint() {
    if (!todoQuick || todoQuick.total === 0) {
      setHint(hintMessage, HINT_MAIN);
      return;
    }
    setHint(hintMessage, HINT_MAIN.slice(0, HINT_MAIN.lastIndexOf(' • ')));
    hintMessage.insertAdjacentHTML('beforeend', ' <span class="hint-sep">•</span> ');
    const widget = document.createElement('span');
    widget.className = 'hint-todo';
    widget.innerHTML = `${listChecks} Todo <b>${todoQuick.done}/${todoQuick.total}</b>`;
    if (todoQuick.open.length > 0) {
      const bubble = document.createElement('div');
      bubble.className = 'hint-todo-bubble';
      const title = document.createElement('div');
      title.className = 'hint-todo-bubble-title';
      title.textContent = 'Unfinished today';
      bubble.appendChild(title);
      for (const name of todoQuick.open) {
        const row = document.createElement('div');
        row.className = 'hint-todo-bubble-task';
        row.textContent = `• ${name}`;
        bubble.appendChild(row);
      }
      widget.appendChild(bubble);
    }
    widget.addEventListener('click', () => {
      commands.enterById('todo');
      enterCommandMode();
      queryInput.value = '';
    });
    hintMessage.appendChild(widget);
  }

  function isHomeHintContext() {
    return !commands.isActive() && !settings.isActive()
      && !search.isTranslateMode() && !search.isClipboardMode()
      && !search.isPrefixHintMode() && !search.isCommandHintMode();
  }

  todoCmd.setOnQuickChange((stat) => {
    todoQuick = stat;
    if (isHomeHintContext()) renderMainHint();
  });

  renderMainHint();

  // Snapshot of the latest results + AI state, used by applyAiLayoutMode()
  // to pick full / two-col / stacked. Mirrors macOS LauncherView resultsRow.
  let lastResults = [];
  let lastAiState = AiState.idle;

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
  confirm.init(document.getElementById('confirm-bar'));
  picked.init(previewPanel, {
    onRemoveItem: (key) => results.removePick(key),
    onClearAll: () => results.clearPicks(),
    onOpenAll: () => keyboard.openAllPicked(),
  });
  commands.init(contentArea, queryInput, {
    onExitMode: exitCommandMode,
    onExecuteCommand: executeCommand,
    onGetIcon: getIcon,
  });
  translatePanel.init(contentArea);
  aiCard.init(aiCardEl);
  aiAnswer.init({
    onChange: (snapshot) => {
      lastAiState = snapshot.state;
      aiCard.update(snapshot);
      applyAiLayoutMode();
      // Switch the results-list empty-state mode in lockstep with the AI
      // state so the right-column "No results" doesn't appear the instant
      // AI flips from streaming to done with an empty suggestions list.
      if (lastAiState !== AiState.idle) {
        results.setEmptyState({ mode: 'ai-suggestion' });
      }
    },
  });
  settings.init(() => {
    queryInput.value = '';
    search.handleQueryInput('');
    queryInput.focus();
    renderMainHint();
  });
  settings.restoreOnStartup();

  // Running apps strip
  runningApps.init(document.getElementById('running-apps-strip'));
  getConfig().then((cfg) => {
    const placement = cfg.entries.find((e) => e.key === 'running_apps_placement');
    const on = !placement || placement.value !== 'none';
    runningApps.setEnabled(on);
    if (on) runningApps.refresh();

    // AI / web answers: default ON to match the default_config.txt setting
    // (and macOS, which ships aiEnabled=true). Honour the persisted value if
    // the user has flipped it. Propagated to both the controller (gates the
    // card) and search.js (gates web suggestions).
    const aiCfg = cfg.entries.find((e) => e.key === 'ai_enabled');
    const aiOn = !aiCfg || aiCfg.value !== 'false';
    aiAnswer.setEnabled(aiOn);
    search.setAiEnabled(aiOn);
  });

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

  // Update right panel when selection changes. Discovery rows have nothing
  // to preview (synthetic, empty path); let the list span full width instead
  // of showing an empty pane (matches macOS LauncherView.swift:872).
  results.setOnSelectionChange((item) => {
    if (results.hasPickedItems()) return;
    if (item && (prefixFromResultId(item.id) != null || commandIdFromResultId(item.id) != null)) {
      previewPanel.hidden = true;
      return;
    }
    previewPanel.hidden = false;
    preview.update(item);
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

  // Wire search -> results. After rendering, drive the AI controller with
  // the LOCAL result count (websuggest: rows don't count: a query that
  // only matches web suggestions is treated as zero local results, which is
  // the macOS knowledge-lookup trigger). Prefix-driven modes (t"/c"/rc"/
  // "/:) own the result area and must NOT trigger AI/web lookups. A query
  // like `t"who is` would otherwise fire isEntityLookup and pull Wikipedia.
  search.setOnResults((items, query) => {
    lastResults = items;
    results.render(items);
    applyAiLayoutMode();
    if (isPrefixedQuery(query)) {
      aiAnswer.cancel();
      return;
    }
    const localCount = items.filter((r) => webSuggestionFromResultId(r.id) == null).length;
    aiAnswer.update(query, localCount);
  });

  // Pick stacked / two-col purely from the local-result count, ignoring the
  // controller state and the suggestion-arrival timing. This is what
  // eliminates the 2-3 second "card width zooms out" shift the user sees:
  // engine returns first (empty), then 2 s later web suggestions arrive,
  // and the OLD logic reacted to every intermediate state with a different
  // mode. Now the only thing that matters is "are there any local rows":
  //   - has local results → stacked (card capped 240 px above rows, both
  //     in col 1; search-bar and rows stay aligned)
  //   - no local results  → two-col (wide card + 320 px suggestion column
  //     on the right; alignment intentionally broken so the answer reads
  //     at a comfortable measure)
  // The 320 px column may be empty briefly while suggestions load, or
  // permanently for queries that get none (e.g. "1+1=?"). Stable layout
  // matters more than the empty column for those edge cases.
  function applyAiLayoutMode() {
    if (!resultsArea) return;
    resultsArea.classList.remove(...AI_LAYOUT_CLASSES);
    if (lastAiState === AiState.idle) return;
    const hasLocal = lastResults.some((r) => webSuggestionFromResultId(r.id) == null);
    resultsArea.classList.add(hasLocal ? AI_LAYOUT_STACKED : AI_LAYOUT_TWO_COL);
  }

  // :cmd <args> live trigger: jumps straight into that command's panel with
  // the rest of the text prefilled (e.g. `:calc 2+2`, `:kill chrome`). Bare
  // `:calc` without a trailing space stays in the discovery menu (matches
  // macOS extractInlineCommand semantics); the user can press Enter on the
  // highlighted row to enter the command with empty input.
  const CMD_PREFIX_MAP = { calc: 'calc', pomo: 'pomo', todo: 'todo', kill: 'kill', shell: 'shell', sys: 'sys' };

  function tryCommandPrefix(value) {
    if (!value.startsWith(':')) return false;
    const rest = value.slice(1);
    const spaceIdx = rest.search(/\s/);
    // No whitespace → not a live trigger; let the discovery menu handle it.
    if (spaceIdx < 0) return false;
    const cmdName = rest.slice(0, spaceIdx);
    const cmdId = CMD_PREFIX_MAP[cmdName.toLowerCase()];
    if (!cmdId) return false;
    const input = rest.slice(spaceIdx + 1);
    commands.enterById(cmdId);
    enterCommandMode();
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
      runningApps.setSuspended(true);
      if (!translatePanel.isActive()) translatePanel.showPlaceholder();
    } else if (search.isClipboardMode()) {
      setHint(hintMessage, HINT_CLIPBOARD);
      resultsList.hidden = false;
      runningApps.setSuspended(false);
      if (runningApps.isEnabled()) runningApps.refresh();
      translatePanel.hide();
    } else if (search.isPrefixHintMode()) {
      setHint(hintMessage, HINT_PREFIX_DISCOVERY);
      resultsList.hidden = false;
      previewPanel.hidden = true;
      runningApps.setSuspended(false);
      if (runningApps.isEnabled()) runningApps.refresh();
      translatePanel.hide();
    } else if (search.isCommandHintMode()) {
      setHint(hintMessage, HINT_COMMAND_DISCOVERY);
      resultsList.hidden = false;
      previewPanel.hidden = true;
      runningApps.setSuspended(false);
      if (runningApps.isEnabled()) runningApps.refresh();
      translatePanel.hide();
    } else {
      renderMainHint();
      resultsList.hidden = false;
      runningApps.setSuspended(false);
      if (runningApps.isEnabled()) runningApps.refresh();
      translatePanel.hide();
      // While the AI card is active, the results list holds web-suggestion
      // rows. Those routinely return empty (DDG rate-limits, transient
      // failures); render nothing instead of "No results" so the right
      // column doesn't shout an error when the left card is working fine.
      const empty = search.isRecentMode()
        ? 'recent'
        : (lastAiState !== AiState.idle ? 'ai-suggestion' : 'default');
      results.setEmptyState({ mode: empty });
    }
  });

  // Click on result row -> open
  resultsList.addEventListener('result-activate', () => {
    const item = results.getSelected();
    if (!item) return;

    // Discovery rows behave the same on click as on Enter: pick a prefix fills
    // the query, pick a command enters that command's panel.
    const hintedPrefix = prefixFromResultId(item.id);
    if (hintedPrefix != null) {
      queryInput.value = hintedPrefix;
      queryInput.focus();
      queryInput.setSelectionRange(hintedPrefix.length, hintedPrefix.length);
      queryInput.dispatchEvent(new Event('input'));
      return;
    }
    const hintedCmd = commandIdFromResultId(item.id);
    if (hintedCmd != null) {
      commands.enterById(hintedCmd);
      enterCommandMode();
      queryInput.value = '';
      return;
    }
    const suggestionText = webSuggestionFromResultId(item.id);
    if (suggestionText != null) {
      const url = `https://www.google.com/search?q=${encodeURIComponent(suggestionText)}`;
      import('./ipc.js').then(({ openPath }) => openPath(url, 'browser', ''));
      return;
    }

    import('./ipc.js').then(({ openPath, recordUsage }) => {
      openPath(item.path, item.kind, item.id);
      const actionMap = { app: 'open_app', file: 'open_file', folder: 'open_folder' };
      recordUsage(item.id, actionMap[item.kind] || 'open_file');
    });
  });

  // When window shown via global hotkey, focus input and select all
  onWindowShown(() => {
    queryInput.focus();
    queryInput.select();
    requestIndexRefresh();
    runningApps.refresh();
    // Re-read todos when nothing would be lost, so the quick view stays
    // fresh across day rollovers and edits from other Look clients.
    todoCmd.reloadIfClean();
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

  // Load resolved quick-folder paths (Desktop, Documents, …). Uses
  // SHGetKnownFolderPath on Windows so OneDrive-redirected folders resolve
  // to their real location; $HOME/<name> on Linux/macOS.
  getQuickFolders().then((folders) => {
    search.setQuickFolders(folders || []);
    search.handleQueryInput('');
  });

  // --- Command mode helpers ---

  function enterCommandMode() {
    resultsList.hidden = true;
    previewPanel.hidden = true;
    runningApps.setSuspended(true);
    // Tear down any active AI card; command mode owns the whole content
    // area, and a stale card would peek through. Matches macOS, which
    // calls aiAnswer.cancel() whenever it switches into command mode.
    aiAnswer.cancel();
    updateCommandHintBar();
    commands.enter();
    commands.setOnCommandChange(updateCommandHintBar);
  }

  function updateCommandHintBar() {
    const cmd = commands.getActiveCommand();
    if (cmd === 'pomo') {
      setHint(hintMessage,
        'Space: Start/pause \u2022 R: Reset \u2022 P: Music \u2022 Esc: Back \u2022 Tab/Ctrl+1-6: Switch');
    } else if (cmd === 'todo') {
      setHint(hintMessage,
        'Ctrl+N: Switch page \u2022 Ctrl+S: Save \u2022 Tab/Ctrl+1-6: Switch \u2022 Esc: Back');
    } else if (cmd === 'kill') {
      setHint(hintMessage,
        'Y: Confirm \u2022 N: Cancel \u2022 Tab/Ctrl+1-6: Switch \u2022 Esc: Back');
    } else if (cmd === 'sys') {
      setHint(hintMessage,
        'Esc: Back \u2022 Tab/Ctrl+1-6: Switch \u2022 Ctrl+/: Command mode \u2022 Ctrl+Shift+,: Settings');
    } else if (cmd === 'calc') {
      setHint(hintMessage,
        'Enter: Evaluate \u2022 Tab: Select \u2022 Ctrl+1-6: Switch \u2022 Esc: Back');
    } else if (cmd === 'shell') {
      setHint(hintMessage,
        'Enter: Run \u2022 Tab: Select \u2022 Ctrl+1-6: Switch \u2022 Esc: Back');
    } else {
      setHint(hintMessage,
        'Enter: Run \u2022 Tab: Select \u2022 Ctrl+1-6: Switch \u2022 Esc: Back');
    }
  }

  function exitCommandMode() {
    queryInput.parentElement.style.display = '';
    resultsList.hidden = false;
    previewPanel.hidden = false;
    translatePanel.hide();
    renderMainHint();
    queryInput.value = '';
    search.handleQueryInput('');
    queryInput.focus();
    runningApps.setSuspended(false);
    if (runningApps.isEnabled()) runningApps.refresh();
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

  // Sync running apps strip + AI when config is reloaded from file. Both
  // settings have live downstream consumers, so propagate on every reload.
  settings.setOnConfigReload((map) => {
    const on = (map.running_apps_placement || 'right') !== 'none';
    runningApps.setEnabled(on);
    if (on) runningApps.refresh();

    const aiOn = map.ai_enabled !== 'false';
    aiAnswer.setEnabled(aiOn);
    search.setAiEnabled(aiOn);
  });

  // Live-update when the Settings → Appearance → Running Apps toggle changes.
  document.addEventListener('look:running-apps-changed', (e) => {
    const enabled = e.detail.enabled;
    runningApps.setEnabled(enabled);
    if (enabled) runningApps.refresh();
  });

  // Live-update when the Settings → Privacy & Logs → Web Answers toggle
  // changes. Propagate immediately so the card and suggestion rows appear or
  // disappear without a config reload.
  document.addEventListener('look:ai-enabled-changed', (e) => {
    const enabled = e.detail.enabled;
    aiAnswer.setEnabled(enabled);
    search.setAiEnabled(enabled);
    // Re-run the current query so the new gate takes effect immediately
    // (drops websuggest rows when disabled, fetches them when enabled).
    search.handleQueryInput(queryInput.value);
  });

  // Expose enterCommandMode and settings for keyboard
  keyboard.setEnterCommandMode(enterCommandMode);
  keyboard.setSettingsMode(settings, contentArea, queryInput.parentElement);
});
