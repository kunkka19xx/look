const SEARCH_DEBOUNCE_MS = 140;

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
let searchDebounce = null;
// True while a backend fuzzy search is scheduled or in flight: the local
// provisional filter may miss port/PID matches the backend still owns.
let searchPending = false;
// Monotonic per-keystroke generation. A search response is only applied when
// its generation is still current, so a superseded request (even for the same
// query text after foo -> bar -> foo) can never overwrite fresher results.
let searchGen = 0;
// True once initial enumeration has returned, so an empty base list reads as a
// terminal "no processes" state rather than a perpetual "Loading...".
let baseLoaded = false;

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
    baseLoaded = false;
    searchPending = false;
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

// `gen` is the generation a backend search was dispatched at; `null` marks the
// base app list (mode entry / post-kill).
export function setProcessList(procs, gen = null) {
    if (gen !== null) {
        // Superseded response (a newer keystroke, retype, or clear advanced the
        // generation): ignore it so only the latest request paints results.
        if (gen !== searchGen) return;
        searchPending = false;
        processList = procs || [];
        filteredProcesses = [...processList];
    } else {
        baseProcessList = procs || [];
        baseLoaded = true;
        processList = [...baseProcessList];
        filterProcesses(input ? input.value.trim() : '');
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
    // Every intent change advances the generation, invalidating any in-flight
    // search whose result has not landed yet.
    searchGen += 1;
    if (!query) {
        clearTimeout(searchDebounce);
        searchPending = false;
        processList = [...baseProcessList];
        filteredProcesses = [...processList];
    } else {
        // Instant provisional list from the loaded apps; the debounced backend
        // search then replaces it with the full fuzzy result (apps first) plus
        // matching non-app processes, including port and PID matches.
        const q = query.toLowerCase();
        filteredProcesses = baseProcessList.filter((p) => p.name.toLowerCase().includes(q));
        clearTimeout(searchDebounce);
        searchPending = true;
        const gen = searchGen;
        searchDebounce = setTimeout(() => {
            if (onExecute) onExecute('kill-search', query, gen);
        }, SEARCH_DEBOUNCE_MS);
    }
    selectedIndex = Math.min(selectedIndex, Math.max(0, filteredProcesses.length - 1));
}

function renderList() {
    listEl.innerHTML = '';
    feedback.textContent = '';

    if (filteredProcesses.length === 0) {
        const query = input ? input.value.trim() : '';
        if (!query) {
            // Before enumeration returns, "Loading..."; once it has, an empty
            // base list is a terminal state, not a perpetual spinner.
            feedback.textContent = baseLoaded ? 'No running processes' : 'Loading...';
        } else {
            // A pending backend search can still return port/PID matches the
            // local name filter missed, so hold off on a definitive miss.
            feedback.textContent = searchPending ? 'Searching...' : 'No matching processes';
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
