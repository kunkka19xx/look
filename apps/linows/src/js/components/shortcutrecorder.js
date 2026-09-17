// Rebindable shortcuts in Settings > Shortcuts. To add one, list it in
// CONFIGURABLE and mark its row in settings.html with `data-shortcut-id`.
import { hotkeyCheck, launcherHotkeyState, setLauncherHotkeyActive } from '../ipc.js';

const CONFIGURABLE = [
    {
        id: 'global.toggleLauncher',
        configKey: 'launcher_hotkey',
        state: launcherHotkeyState,
        setActive: setLauncherHotkeyActive,
    },
];

const LISTENING = 'Press a shortcut, Esc to cancel';
const UNKNOWN_KEY = 'That key cannot be used';
const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'OS']);
const LETTER = /^[a-z]$/i;

// Keyed by config key.
const states = new Map();
const pending = new Map();
let recording = null;
let screen = null;

export function init(root) {
    screen = root;
    for (const shortcut of CONFIGURABLE) {
        const row = rowFor(shortcut);
        row.querySelector('kbd').addEventListener('click', () => {
            if (!states.get(shortcut.configKey)?.configurable) return;
            if (recording?.shortcut === shortcut) stop();
            else start(shortcut);
        });
        row.querySelector('.settings-shortcut-reset').addEventListener('click', () => {
            const state = states.get(shortcut.configKey);
            pending.set(shortcut.configKey, {
                spec: state.default_spec,
                display: state.default_display,
            });
            render();
        });
    }
}

export async function refresh() {
    for (const shortcut of CONFIGURABLE) {
        states.set(shortcut.configKey, await shortcut.state().catch(() => null));
    }
    render();
}

export function discardPending() {
    stop();
    pending.clear();
    render();
}

export function pendingUpdates() {
    return Object.fromEntries([...pending].map(([key, { spec }]) => [key, spec]));
}

// Call once the pending values are in the config file.
export async function applySaved() {
    stop();
    const saved = CONFIGURABLE.filter((s) => pending.has(s.configKey));
    pending.clear();
    await Promise.all(saved.map((s) => s.setActive(true)));
    await refresh();
}

export { stop as cancel };

function rowFor(shortcut) {
    return screen.querySelector(`[data-shortcut-id="${shortcut.id}"]`);
}

function start(shortcut) {
    stop();
    recording = { shortcut, error: null, listener: (e) => record(shortcut, e) };
    // Window capture runs before the launcher's document handler.
    window.addEventListener('keydown', recording.listener, true);
    shortcut.setActive(false).catch(console.error);
    render();
}

function stop() {
    if (!recording) return;
    window.removeEventListener('keydown', recording.listener, true);
    recording.shortcut.setActive(true).catch(console.error);
    recording = null;
    render();
}

async function record(shortcut, e) {
    e.preventDefault();
    e.stopImmediatePropagation();
    if (MODIFIER_KEYS.has(e.key)) return;
    const mods = [e.ctrlKey && 'ctrl', e.altKey && 'alt', e.shiftKey && 'shift', e.metaKey && 'win'];
    if (e.key === 'Escape' && !mods.some(Boolean)) {
        stop();
        return;
    }

    const spec = [...mods.filter(Boolean), keyToken(e)].join('+');
    const check = await hotkeyCheck(spec).catch(() => null);
    if (recording?.shortcut !== shortcut) return;
    if (check?.error || !check) {
        recording.error = check?.error ?? UNKNOWN_KEY;
        render();
        return;
    }
    pending.set(shortcut.configKey, { spec: check.spec, display: check.display });
    stop();
}

// Letters follow the printed layout; digits and named keys the physical key.
function keyToken(e) {
    if (LETTER.test(e.key)) return e.key.toLowerCase();
    return e.code.replace(/^(Key|Digit)/, '').toLowerCase();
}

function render() {
    let unsaved = 0;
    for (const shortcut of CONFIGURABLE) {
        const state = states.get(shortcut.configKey);
        if (!state) continue;
        const row = rowFor(shortcut);
        const isRecording = recording?.shortcut === shortcut;
        const shown = pending.get(shortcut.configKey)?.display ?? state.display;
        const changed = shown !== state.display;
        if (changed) unsaved += 1;

        const kbd = row.querySelector('kbd');
        kbd.textContent = isRecording ? LISTENING : shown;
        kbd.classList.toggle('settings-shortcut-recorder', state.configurable);
        kbd.classList.toggle('settings-shortcut-active', isRecording || changed);

        row.querySelector('.settings-shortcut-reset').hidden =
            !state.configurable || isRecording || shown === state.default_display;

        const error = row.querySelector('.settings-shortcut-error');
        error.textContent = (isRecording && recording.error) || '';
        error.hidden = !error.textContent;
    }

    const notice = screen.querySelector('#settings-shortcuts-notice');
    notice.textContent = `${unsaved} shortcut change${unsaved === 1 ? '' : 's'} pending. Save Config to apply.`;
    notice.hidden = unsaved === 0;
}
