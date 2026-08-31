import * as results from './components/results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './components/preview.js';
import * as actionmenu from './components/actionmenu.js';
import * as picked from './components/picked.js';
import * as banner from './components/banner.js';
import * as health from './components/health.js';
import * as confirm from './components/confirm.js';
import * as commands from './screens/commands/index.js';
import * as todoCmd from './screens/commands/todo.js';
import { listChecks } from './icons.js';
import * as settings from './screens/settings.js';
import { mountUpdateWidget } from './screens/update_widget.js';
import * as translatePanel from './components/translate.js';
import * as runningApps from './components/running-apps.js';
import * as superactions from './components/superactions.js';
import * as sourceblocks from './components/sourceblocks.js';
import * as levels from './levels.js';
import * as smoothcaret from './components/smoothcaret.js';
import * as platform from './platform.js';
import * as motion from './motion.js';
import * as aiAnswer from './components/ai-answer.js';
import { State as AiState } from './components/ai-answer.js';
import * as aiCard from './components/ai-answer-card.js';
import * as layout from './layout.js';
import { load } from './html-loader.js';
import {
    onWindowShown,
    onWindowHidden,
    confirmHide,
    onIndexReady,
    requestIndexRefresh,
    getQuickFolders,
    copyFilesToClipboard,
    evalCalc,
    runShellCommand,
    getSystemInfo,
    listProcesses,
    searchKillTargets,
    killProcess,
    getIcon,
    copyToClipboard,
    deleteClipboardEntry,
    isDevBuild,
    getConfig,
} from './ipc.js';
import {
    COMMAND_ENTRIES,
    isCommandId,
    prefixFromResultId,
    commandIdFromResultId,
    webSuggestionFromResultId,
    webUrlFromResultId,
    classifyResultId,
    isPrefixedQuery,
} from './catalog.js';

// Item count and structure mirror the macOS app's `LauncherView.hintItems`
// (apps/macos/.../LauncherView.swift:302) so both platforms surface the same
// shortcuts in the same modes. Style stays per-platform: linows uses the
// colon + bold-bullet format, macOS keeps its space-separated form.
//
// Every line here has to fit the left card footer in one row when the panes
// float, which is the narrowest place a hint is shown. That budget is what
// dropped "Ctrl+F: Reveal" and "Ctrl+/: Command mode" from the home hints, and
// what keeps the clipboard hint to its first two items - all three still work
// and are still listed in Settings > Shortcuts.
const HINT_MAIN = 'Enter: Open \u2022 Ctrl+K: Actions \u2022 Ctrl+H: Help';
// Inside a level the way out is the thing to say.
const HINT_LEVEL = 'Enter: Open \u2022 Ctrl+K: Actions \u2022 Esc: Back';
const HINT_TRANSLATE = 'Enter: Translate \u2022 Copy per result \u2022 Ctrl+H: Help';
const HINT_CLIPBOARD = 'Enter: Copy clip \u2022 Ctrl+D: Remove clip';
const HINT_PROCESS = 'Enter: CPU \u2022 Ctrl+D: Kill \u2022 Ctrl+C: Copy PID';
// Discovery-menu hints \u2014 mirror macOS prefixSuggestion / commandSuggestion
// hint bars (LauncherView.swift hintItems).
const HINT_PREFIX_DISCOVERY =
    'Enter: Pick prefix \u2022 Up/Down: Move \u2022 Esc: Clear \u2022 Ctrl+H: Help';
const HINT_COMMAND_DISCOVERY =
    'Enter: Run command \u2022 Up/Down: Move \u2022 Esc: Clear \u2022 Ctrl+H: Help';

// "Ctrl+1-7: Switch", derived from the catalog so a new command can't leave the
// hint stale (mirrors the macOS commandSwitchHint).
const SWITCH_HINT = `Ctrl+1-${COMMAND_ENTRIES.length}: Switch`;

// Per-command hint lines while command mode is active; `shell` doubles as
// the fallback for commands without a dedicated line.
const COMMAND_HINTS = {
    pomo: `Space: Start/pause \u2022 R: Reset \u2022 P: Music \u2022 Esc: Back \u2022 Tab/${SWITCH_HINT}`,
    todo: `Ctrl+N: Switch page \u2022 Ctrl+S: Save \u2022 Tab/${SWITCH_HINT} \u2022 Esc: Back`,
    speed: `R: Rerun \u2022 E: Show IP \u2022 Esc: Back \u2022 Tab/${SWITCH_HINT}`,
    kill: `Y: Confirm \u2022 N: Cancel \u2022 Tab/${SWITCH_HINT} \u2022 Esc: Back`,
    sys: `Esc: Back \u2022 Tab/${SWITCH_HINT} \u2022 Ctrl+/: Command mode \u2022 Ctrl+Shift+,: Settings`,
    calc: `Enter: Evaluate \u2022 Tab: Select \u2022 ${SWITCH_HINT} \u2022 Esc: Back`,
    shell: `Enter: Run \u2022 Tab: Select \u2022 ${SWITCH_HINT} \u2022 Esc: Back`,
};

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
// Two-col: answer card left, suggestion list right, equal columns.
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

    // The Ctrl+1..N rows in Help and Settings, filled from the same catalog the
    // bindings come from, so neither list can go stale behind a new command.
    for (const screen of ['help', 'settings']) {
        document.getElementById(`${screen}-cmd-keys`).textContent =
            `Ctrl+1..${COMMAND_ENTRIES.length}`;
        document.getElementById(`${screen}-cmd-list`).textContent =
            `Switch to ${COMMAND_ENTRIES.map((cmd) => `/${cmd.id}`).join(', ')}`;
    }

    // About / version + update-status widget, shared between Settings and Help.
    // Settings gets an "About" header label; Help mounts the widget bare so the
    // screen title above already serves as its heading (mirrors macOS layout).
    mountUpdateWidget(document.getElementById('settings-about'), { label: 'About' });
    mountUpdateWidget(document.getElementById('help-update'));

    // Hint bar: at the bottom, shared by all screens. In the floating grid,
    // layout.js relocates the message span into the left card footer and the
    // copyright into the right card footer.
    app.insertAdjacentHTML(
        'beforeend',
        `<div class="hint-bar" id="hint-bar"><span id="hint-message"></span><span class="hint-bar-copy">\u00A9 2026 by <a class="hint-bar-link" href="#">Kunkka</a></span></div>`,
    );

    // Load command panels into cmd-main. Each panel's template is named after
    // its command id, so the catalog drives the list.
    const cmdMain = document.getElementById('cmd-main');
    await Promise.all(
        COMMAND_ENTRIES.map((cmd) => load(`html/screens/commands/${cmd.id}.html`, cmdMain)),
    );

    // DOM refs
    const queryInput = document.getElementById('query');
    const resultsList = document.getElementById('results-list');
    const previewPanel = document.getElementById('preview-panel');
    const hintBar = document.getElementById('hint-bar');
    const hintMessage = document.getElementById('hint-message');
    const contentArea = document.getElementById('search-content');
    const resultsArea = document.getElementById('results-area');
    const aiCardEl = document.getElementById('ai-answer-card');
    const helpScreen = document.getElementById('help-screen');
    const previewCol = document.getElementById('preview-col');
    const breadcrumbEl = document.getElementById('search-breadcrumb');
    const placeholderEl = document.querySelector('.search-placeholder');
    const previewFooter = document.getElementById('preview-footer');

    // Floating "inner-gap" layout state (classes on .launcher-window)
    layout.init();

    // Entrance motion; replays on every summon (see the window-shown handler).
    motion.init(app);
    motion.playReveal();
    layout.initHints({
        hintBar,
        hintMessage,
        copyright: hintBar.querySelector('.hint-bar-copy'),
        leftFooter: document.getElementById('results-footer'),
        rightFooter: previewFooter,
    });

    // Todo quick view: when today has tasks, the last main-hint item
    // ("Ctrl+H: Help") is swapped for a clickable "Todo X/Y" stat with an
    // "Unfinished today" hover bubble, so the line stays one row. Mirrors macOS
    // HintBar.TodoQuickView: home screen only, hidden when today is empty.
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
        return (
            !levels.isActive() &&
            !commands.isActive() &&
            !settings.isActive() &&
            helpScreen?.hidden !== false &&
            !search.isTranslateMode() &&
            !search.isClipboardMode() &&
            !search.isPrefixHintMode() &&
            !search.isCommandHintMode()
        );
    }

    // The empty-state control strip (super actions) stands in for the results
    // row whenever the query is empty on the bare home screen. Single source of
    // truth: any transition that changes the query or the active screen calls
    // this, and the strip renders itself only when it should. When the strip
    // yields to results (first keystroke from empty), ease the results list in.
    function syncControlStrip() {
        const was = superactions.isVisible();
        superactions.setVisible(queryInput.value === '' && isHomeHintContext());
        if (was && !superactions.isVisible()) playResultsEnter();
    }

    // Replay the results-list entrance animation once. Restarting needs the
    // class dropped and a reflow forced before it is re-added.
    function playResultsEnter() {
        resultsList.classList.remove('is-entering');
        void resultsList.offsetWidth;
        resultsList.classList.add('is-entering');
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
    // Smooth caret on the main search input and every command-mode input bar.
    smoothcaret.attach(queryInput);
    for (const input of cmdMain.querySelectorAll('.cmd-input-bar input')) {
        smoothcaret.attach(input);
    }
    preview.init(previewPanel);
    actionmenu.init(previewPanel, queryInput);
    // What each declared block asked to be drawn as, read once: rows render
    // synchronously and a miss costs them their icon until it lands.
    sourceblocks.prefill();
    // `{query}` is whatever is typed right now, at whatever level.
    sourceblocks.setQueryProvider(() => queryInput.value);
    banner.init(document.getElementById('banner'));
    health.init();
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
    function syncSearchSurface() {
        const translating = search.isTranslateMode();
        resultsList.hidden = translating;
        runningApps.setSuspended(translating);

        if (translating) {
            previewPanel.hidden = true;
            if (!translatePanel.isActive()) translatePanel.showPlaceholder();
            return;
        }

        translatePanel.hide();
        previewPanel.hidden = false;
    }

    // Shared "back to the empty home screen" reset, used when leaving
    // settings or command mode.
    function resetHomeQuery() {
        // A level is a place inside the launcher, and this is the way home.
        if (levels.isActive()) {
            levels.clear();
            syncBreadcrumb();
        }
        queryInput.value = '';
        search.handleQueryInput('');
        layout.setQuery({ empty: true, translate: false });
        syncSearchSurface();
        renderMainHint();
        syncControlStrip();
        queryInput.focus();
        smoothcaret.refresh(queryInput);
    }

    settings.init(resetHomeQuery);
    settings.restoreOnStartup();

    // Empty-state super-actions control strip
    superactions.init(document.getElementById('control-strip'));

    // Running apps strip
    runningApps.init(document.getElementById('running-apps-strip'));
    getConfig().then((cfg) => {
        const placement = cfg.entries.find((e) => e.key === 'running_apps_placement');
        const on = !placement || placement.value !== 'none';
        runningApps.setEnabled(on);
        if (on) runningApps.refresh();

        // Super actions launchpad: default ON. A disabled strip stays hidden on
        // the empty home screen and its accelerators go inert (see superactions).
        const superCfg = cfg.entries.find((e) => e.key === 'super_actions_enabled');
        superactions.setEnabled(!superCfg || superCfg.value !== 'false');
        syncControlStrip();

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
        if (
            item &&
            (prefixFromResultId(item.id) != null || commandIdFromResultId(item.id) != null)
        ) {
            // Nothing to preview, so preview.update never runs and cannot close
            // the menu for us.
            actionmenu.close();
            previewPanel.hidden = true;
            return;
        }
        // The menu lists the selected row's verbs and cannot outlive that row;
        // preview.update closes it exactly when the row changes. Closing here
        // would also fire on a re-render that re-selects the SAME row, which is
        // every progressive publish (engine, URL rows, web suggestions).
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
                    .then(() =>
                        banner.show(
                            `Picked ${pickedItems.length} item(s)`,
                            'success',
                            BANNER_DURATION_SHORT,
                        ),
                    )
                    .catch(() => banner.show('Pick failed', 'error', BANNER_DURATION_MEDIUM));
            } else {
                banner.show(
                    `Picked ${pickedItems.length} item(s)`,
                    'success',
                    BANNER_DURATION_SHORT,
                );
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
        // A level owns the list. A search that was in flight when the user
        // descended must not paint the index over it.
        if (levels.isActive()) return;
        lastResults = items;
        results.render(items, query);
        applyAiLayoutMode();
        // Recent-empty renders as one wide card, which sends the hint bar back
        // to the bottom while the panes float (macOS showsFloatingGrid).
        layout.setRecentEmpty(search.isRecentMode() && items.length === 0);
        // Clipboard with no clips: the same two-card grid as normal results -
        // "Clipboard History" info on the left, "How to use" on the right
        // (macOS ClipboardEmptyInfoView / ClipboardEmptyHelpView).
        if (search.isClipboardMode() && items.length === 0) {
            previewPanel.hidden = false;
            preview.showClipboardHelp();
        }
        if (isPrefixedQuery(query)) {
            aiAnswer.cancel();
            return;
        }
        const localCount = items.filter((r) => !isSyntheticSuggestionRow(r)).length;
        aiAnswer.update(query, localCount);
    });

    // Synthesized suggestion rows (Google autocomplete, URL open/history) are
    // not local results: they never count toward the AI knowledge-lookup
    // trigger or the stacked/two-col layout choice.
    function isSyntheticSuggestionRow(r) {
        return webSuggestionFromResultId(r.id) != null || webUrlFromResultId(r.id) != null;
    }

    // Pick stacked / two-col purely from the local-result count, ignoring the
    // controller state and the suggestion-arrival timing. This is what
    // eliminates the 2-3 second "card width zooms out" shift the user sees:
    // engine returns first (empty), then 2 s later web suggestions arrive,
    // and the OLD logic reacted to every intermediate state with a different
    // mode. Now the only thing that matters is "are there any local rows":
    //   - has local results → stacked (card capped 240 px above rows, both
    //     in col 1; search-bar and rows stay aligned)
    //   - no local results  → two-col (answer card left, suggestion list
    //     right, equal columns - the same two-pane grid as normal results,
    //     matching macOS twoPaneGrid)
    // The suggestion column may be empty briefly while suggestions load, or
    // permanently for queries that get none (e.g. "1+1=?"). Stable layout
    // matters more than the empty column for those edge cases.
    function applyAiLayoutMode() {
        if (!resultsArea) return;
        resultsArea.classList.remove(...AI_LAYOUT_CLASSES);
        let mode = null;
        if (lastAiState !== AiState.idle) {
            const hasLocal = lastResults.some((r) => !isSyntheticSuggestionRow(r));
            mode = hasLocal ? AI_LAYOUT_STACKED : AI_LAYOUT_TWO_COL;
            resultsArea.classList.add(mode);
        }
        // Two-col hosts the suggestion list in the right pane; every other mode
        // keeps it under the search column. Same element either way, so
        // selection, keyboard nav and the results.js container ref all survive
        // the move (which only happens on an actual mode transition).
        if (mode === AI_LAYOUT_TWO_COL) {
            if (resultsList.parentElement !== previewCol) {
                previewCol.insertBefore(resultsList, previewFooter);
            }
        } else if (resultsList.parentElement !== resultsArea) {
            resultsArea.appendChild(resultsList);
        }
    }

    // :cmd <args> live trigger: jumps straight into that command's panel with
    // the rest of the text prefilled (e.g. `:calc 2+2`, `:kill chrome`). Bare
    // `:calc` without a trailing space stays in the discovery menu (matches
    // macOS extractInlineCommand semantics); the user can press Enter on the
    // highlighted row to enter the command with empty input.
    function tryCommandPrefix(value) {
        if (!value.startsWith(':')) return false;
        const rest = value.slice(1);
        const spaceIdx = rest.search(/\s/);
        // No whitespace → not a live trigger; let the discovery menu handle it.
        if (spaceIdx < 0) return false;
        const cmdId = rest.slice(0, spaceIdx).toLowerCase();
        if (!isCommandId(cmdId)) return false;
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

    // Search on input. Translate owns the whole content row; every other mode
    // shows the results list, wakes the running-apps strip and then only
    // differs in hint text, preview visibility and empty-state flavor.
    queryInput.addEventListener('input', (e) => {
        const value = e.target.value;

        // A level owns the result list: its rows are produced live and are not
        // in the index, so typing filters them rather than searching. No
        // cross-level search - reaching the index again is one Escape.
        if (levels.isActive()) {
            actionmenu.close();
            renderLevel();
            return;
        }

        if (tryCommandPrefix(value)) return;

        // Typing re-renders the list the menu hangs off, and the modes below
        // take the preview panel away from under it entirely.
        actionmenu.close();

        search.handleQueryInput(value);
        const translating = search.isTranslateMode();
        layout.setQuery({ empty: layout.isEmptyQuery(value), translate: translating });
        syncControlStrip();
        syncSearchSurface();

        if (translating) {
            setHint(hintMessage, HINT_TRANSLATE);
            return;
        }

        if (search.isClipboardMode()) {
            setHint(hintMessage, HINT_CLIPBOARD);
            results.setEmptyState({ mode: 'clipboard' });
        } else if (search.isProcessMode()) {
            setHint(hintMessage, HINT_PROCESS);
            results.setEmptyState({ mode: 'default' });
        } else if (search.isPrefixHintMode() || search.isCommandHintMode()) {
            setHint(
                hintMessage,
                search.isPrefixHintMode() ? HINT_PREFIX_DISCOVERY : HINT_COMMAND_DISCOVERY,
            );
            previewPanel.hidden = true;
        } else {
            renderMainHint();
            // While the AI card is active, the results list holds web-suggestion
            // rows. Those routinely return empty (DDG rate-limits, transient
            // failures); render nothing instead of "No results" so the right
            // column doesn't shout an error when the left card is working fine.
            const empty = search.isRecentMode()
                ? 'recent'
                : lastAiState !== AiState.idle
                  ? 'ai-suggestion'
                  : 'default';
            results.setEmptyState({ mode: empty });
        }
    });

    // Click on result row -> open
    resultsList.addEventListener('result-activate', () => {
        const item = results.getSelected();
        if (!item) return;

        // Discovery/synthetic rows behave the same on click as on Enter
        // (keyboard.js openSelected): pick a prefix fills the query, pick a
        // command enters that command's panel, calc copies the answer, a
        // web-suggestion/URL row opens in the browser.
        const classified = classifyResultId(item.id);
        switch (classified?.kind) {
            case 'prefixSuggestion':
                queryInput.value = classified.prefix;
                queryInput.focus();
                queryInput.setSelectionRange(classified.prefix.length, classified.prefix.length);
                queryInput.dispatchEvent(new Event('input'));
                return;
            case 'commandSuggestion':
                commands.enterById(classified.commandId);
                enterCommandMode();
                queryInput.value = '';
                return;
            case 'calc':
                import('./ipc.js').then(({ copyToClipboardLabeled, hideWindow }) =>
                    copyToClipboardLabeled(classified.raw, `${item.calcExpr} = ${item.title}`).then(
                        hideWindow,
                    ),
                );
                return;
            case 'webSuggestion': {
                const url = `https://www.google.com/search?q=${encodeURIComponent(classified.text)}`;
                import('./ipc.js').then(({ openPath }) => openPath(url, 'browser', ''));
                return;
            }
            case 'webUrl':
                import('./ipc.js').then(({ openPath, recordUrlHit }) => {
                    openPath(classified.url, 'browser', '');
                    recordUrlHit(classified.url);
                });
                return;
        }
        // Process row has no path to open; a click measures CPU like Enter.
        if (item.kind === 'process') {
            preview.measureCpu();
            return;
        }

        // A block's row runs what the block declared, by click as by Enter.
        if (sourceblocks.isSourceRow(item.id)) {
            sourceblocks.activateRow(item);
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
        smoothcaret.refresh(queryInput);
        requestIndexRefresh();
        // A block edited while Look was away takes effect on this open.
        sourceblocks.prefill();
        runningApps.refresh();
        // Quick-action state is cached from the last render; the system may have
        // changed while hidden (Bluetooth flipped elsewhere), so re-read it.
        preview.refreshQuickActions();
        // Re-read todos when nothing would be lost, so the quick view stays
        // fresh across day rollovers and edits from other Look clients.
        todoCmd.reloadIfClean();
        // The window keeps its query across hide/show; re-assert the strip so an
        // empty-query re-open lands back on the launchpad, replaying its
        // entrance so it animates in each time Look is summoned.
        syncControlStrip();
        superactions.replayEnter();
        // Last: the reveal is the frame the rest of the cascade lands in.
        motion.playReveal();
    });

    // Hold the launchpad at its entrance-start pose before hiding, so the stale
    // buffer the compositor presents on the next summon matches frame 0 instead
    // of flashing the full strip then rewinding. Rust's `window-hidden` is the
    // primary trigger (WebView2 doesn't reliably fire visibilitychange on a
    // native hide); visibilitychange stays as a WebKitGTK fallback.
    onWindowHidden((event) => {
        // A level does not survive the launcher closing (§2.10): what survives
        // is the ranking. Dropped here so the next summon opens on the index.
        // Cleared even with no level up: a target that produces rows may still
        // be running, and its answer must not open a level on the next summon.
        if (levels.isActive()) resetHomeQuery();
        else levels.clear();
        superactions.armEntrance();
        motion.armReveal();
        // The next summon keeps the query and the selection, but an open menu
        // is a question the user already walked away from.
        actionmenu.close();
        // Rust holds the window up until this lands. Two frames: the first
        // callback runs before the armed frame is painted, the second after.
        // The payload keys the dismissal. A failed invoke falls back to Rust's
        // timeout.
        requestAnimationFrame(() =>
            requestAnimationFrame(() => {
                confirmHide(event.payload).catch(() => {});
            }),
        );
    });
    document.addEventListener('visibilitychange', () => {
        if (document.hidden) {
            superactions.armEntrance();
            motion.armReveal();
        } else {
            // A show that never reaches window-shown must still un-arm.
            motion.playReveal();
        }
    });

    onIndexReady(() => {
        // A level's rows are not in the index, and its query filters them.
        if (levels.isActive()) return;
        search.handleQueryInput(queryInput.value);
    });

    // Guard: if focus drifts to another element on the main screen,
    // pull it back to the search input.
    document.addEventListener('focusin', (e) => {
        if (
            e.target !== queryInput &&
            !commands.isActive() &&
            !settings.isActive() &&
            !helpScreen?.contains(e.target)
        ) {
            queryInput.focus();
        }
    });

    // Load resolved quick-folder paths (Desktop, Documents, …). Uses
    // SHGetKnownFolderPath on Windows so OneDrive-redirected folders resolve
    // to their real location; $HOME/<name> on Linux/macOS.
    getQuickFolders().then((folders) => {
        search.setQuickFolders(folders || []);
        search.handleQueryInput('');
        syncControlStrip();
    });

    // --- Levels (drill-down into a `then` target that lists, §2.10) ---

    // The rows of the current level, filtered by what is typed at it. Filtered,
    // never re-ranked: the block's author ordered them, and they are not
    // competing with apps and files for the top slot.
    function renderLevel() {
        previewPanel.hidden = false;
        results.render(levels.rows(queryInput.value), queryInput.value);
    }

    function syncBreadcrumb() {
        const crumbs = levels.breadcrumb();
        breadcrumbEl.hidden = crumbs.length === 0;
        breadcrumbEl.textContent = crumbs.join('  \u203A  ');
        // A level's empty query is for filtering, not an invitation to search.
        // Hidden here rather than in CSS: the placeholder's own rule keys on
        // #query, which outranks anything a sibling selector could say.
        placeholderEl.hidden = crumbs.length > 0;
    }

    // A level was pushed: the query that got here means nothing now, and the
    // launcher's other modes have nothing to say inside one.
    function enterLevel() {
        actionmenu.close();
        aiAnswer.cancel();
        translatePanel.hide();
        resultsList.hidden = false;
        queryInput.value = '';
        smoothcaret.refresh(queryInput);
        layout.setQuery({ empty: false, translate: false });
        syncControlStrip();
        syncBreadcrumb();
        setHint(hintMessage, HINT_LEVEL);
        renderLevel();
    }

    // Escape: back one level, with the query and selection it opened from, so
    // descending and coming back is free rather than a re-search.
    function popLevel() {
        const left = levels.pop();
        if (!left) return;
        queryInput.value = left.restoredQuery;
        smoothcaret.refresh(queryInput);
        // Setting the query seeds the selection from the first row, so the
        // restore waits for whichever pass has the rows.
        results.restoreSelection(left.restoredSelectionId, left.restoredQuery);
        syncBreadcrumb();

        if (levels.isActive()) {
            setHint(hintMessage, HINT_LEVEL);
            renderLevel();
            return;
        }
        // Back to the index, where that query is a search again.
        search.handleQueryInput(left.restoredQuery);
        layout.setQuery({
            empty: layout.isEmptyQuery(left.restoredQuery),
            translate: false,
        });
        syncControlStrip();
        renderMainHint();
    }

    levels.setOnEnter(enterLevel);
    keyboard.setPopLevel(popLevel);

    // --- Command mode helpers ---

    function enterCommandMode() {
        // Command mode owns the whole content area, and an open menu would go
        // on eating the keys it navigates with.
        actionmenu.close();
        superactions.setVisible(false);
        resultsList.hidden = true;
        previewPanel.hidden = true;
        runningApps.setSuspended(true);
        // Tear down any active AI card; command mode owns the whole content
        // area, and a stale card would peek through. Matches macOS, which
        // calls aiAnswer.cancel() whenever it switches into command mode.
        aiAnswer.cancel();
        layout.setModal('command', true);
        updateCommandHintBar();
        commands.enter();
        commands.setOnCommandChange(updateCommandHintBar);
    }

    function updateCommandHintBar() {
        const cmd = commands.getActiveCommand();
        setHint(hintMessage, COMMAND_HINTS[cmd] || COMMAND_HINTS.shell);
    }

    function exitCommandMode() {
        queryInput.parentElement.style.display = '';
        resultsList.hidden = false;
        previewPanel.hidden = false;
        translatePanel.hide();
        layout.setModal('command', false);
        resetHomeQuery();
        runningApps.setSuspended(false);
    }

    async function executeCommand(cmdId, input, gen) {
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

        if (cmdId === 'kill-search') {
            try {
                const targets = await searchKillTargets(input);
                commands.setProcessList(targets, gen);
            } catch (err) {
                commands.showFeedback(err || 'Search failed', true);
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

        superactions.setEnabled(map.super_actions_enabled !== 'false');
        syncControlStrip();

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

    // Live-update when the Settings → Appearance → Super Actions toggle changes.
    document.addEventListener('look:super-actions-changed', (e) => {
        superactions.setEnabled(e.detail.enabled);
        syncControlStrip();
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
    // Settings and Help open/close from keyboard.js; let it re-assert the
    // control strip so it steps aside for those screens and returns after.
    keyboard.setSyncHome(syncControlStrip);
});
