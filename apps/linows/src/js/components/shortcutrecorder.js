// Rebindable shortcuts in Settings > Shortcuts. To add one, list it in
// CONFIGURABLE and mark its row in settings.html with `data-shortcut-id`.
import {
    applyLauncherHotkey,
    hotkeyCheck,
    launcherHotkeyState,
    suspendLauncherHotkey,
} from '../ipc.js';
import { isModifierOnly, specFor } from '../hotkeyspec.js';

const CONFIGURABLE = [
    {
        id: 'global.toggleLauncher',
        configKey: 'launcher_hotkey',
        state: launcherHotkeyState,
        suspend: suspendLauncherHotkey,
        resume: applyLauncherHotkey,
    },
];

const LISTENING = 'Press a shortcut, Esc to cancel';
const UNKNOWN_KEY = 'That key cannot be used';

const CLASS_RECORDER = 'settings-shortcut-recorder';
const CLASS_RECORDING = 'settings-shortcut-recording';
const CLASS_UNSAVED = 'settings-shortcut-unsaved';

// All keyed by config key.
const states = new Map();
const pending = new Map();
let recording = null;
let screen = null;

export function init(root) {
    screen = root;
    for (const shortcut of CONFIGURABLE) {
        const row = rowFor(shortcut);
        row?.querySelector('kbd').addEventListener('click', () => {
            if (!states.get(shortcut.configKey)?.configurable) return;
            if (recording?.shortcut === shortcut) stop();
            else start(shortcut);
        });
        row?.querySelector('.settings-shortcut-reset').addEventListener('click', () => {
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
    await Promise.all(
        CONFIGURABLE.map(async (shortcut) => {
            try {
                states.set(shortcut.configKey, await shortcut.state());
            } catch (err) {
                console.error(`Failed to read ${shortcut.configKey}:`, err);
            }
        }),
    );
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
    await Promise.all(saved.map((s) => s.resume()));
    await refresh();
}

export { stop as cancel };

function rowFor(shortcut) {
    return screen?.querySelector(`[data-shortcut-id="${shortcut.id}"]`);
}

function start(shortcut) {
    stop();
    recording = { shortcut, error: null, listener: (e) => record(shortcut, e) };
    // Window capture runs before the launcher's document handler.
    window.addEventListener('keydown', recording.listener, true);
    shortcut.suspend().catch((err) => console.error('Failed to pause shortcut:', err));
    render();
}

function stop() {
    if (!recording) return;
    const { shortcut, listener } = recording;
    window.removeEventListener('keydown', listener, true);
    recording = null;
    shortcut.resume().catch((err) => console.error('Failed to restore shortcut:', err));
    render();
}

async function record(shortcut, e) {
    e.preventDefault();
    e.stopImmediatePropagation();
    if (isModifierOnly(e)) return;
    const bare = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
    if (e.key === 'Escape' && bare) {
        stop();
        return;
    }

    const check = await hotkeyCheck(specFor(e)).catch(() => null);
    if (recording?.shortcut !== shortcut) return;
    if (!check || check.error) {
        recording.error = check?.error ?? UNKNOWN_KEY;
        render();
        return;
    }
    pending.set(shortcut.configKey, { spec: check.spec, display: check.display });
    stop();
}

function render() {
    if (!screen) return;
    let unsavedCount = 0;
    for (const shortcut of CONFIGURABLE) {
        const row = rowFor(shortcut);
        const state = states.get(shortcut.configKey);
        if (!row || !state) continue;

        const isRecording = recording?.shortcut === shortcut;
        const shown = pending.get(shortcut.configKey)?.display ?? state.display;
        const unsaved = shown !== state.display;
        if (unsaved) unsavedCount += 1;

        const kbd = row.querySelector('kbd');
        kbd.textContent = isRecording ? LISTENING : shown;
        kbd.classList.toggle(CLASS_RECORDER, state.configurable);
        kbd.classList.toggle(CLASS_RECORDING, isRecording);
        kbd.classList.toggle(CLASS_UNSAVED, unsaved && !isRecording);

        row.querySelector('.settings-shortcut-reset').hidden =
            !state.configurable || isRecording || shown === state.default_display;

        const error = screen.querySelector(`[data-shortcut-error="${shortcut.id}"]`);
        const message = isRecording ? recording.error : null;
        error.textContent = message ?? '';
        error.hidden = !message;
    }

    const notice = screen.querySelector('#settings-shortcuts-notice');
    const plural = unsavedCount === 1 ? '' : 's';
    notice.textContent = `${unsavedCount} shortcut change${plural} pending. Save Config to apply.`;
    notice.hidden = unsavedCount === 0;
}
