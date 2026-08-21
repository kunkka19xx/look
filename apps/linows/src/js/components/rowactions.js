// The verbs a row with a path offers, and the tools that carry them out.
//
// Mirrors macOS LauncherView+RowActions and LauncherView+Tools: one declarative
// table plus one eligibility rule, read by both the Ctrl+K menu and the chords,
// so the two can never drift. Composition lives in core (look-tools) and the
// wording of an action that cannot run comes back with it - nothing here writes
// a message about a missing tool.

import { toolActions, performToolAction } from '../ipc.js';
import * as results from './results.js';
import * as banner from './banner.js';
import { isSyntheticResultId } from '../catalog.js';
import { systemFileManager } from '../platform.js';

// Core's action ids (look_tools::Action::id).
export const EDIT = 'edit';
export const TERMINAL = 'terminal';
export const REVEAL = 'reveal';

// Namespaces the menu ids so they cannot be confused with the core action ids
// above, which are a different vocabulary living in the same file.
const PREFIX = 'rowaction:';

const BANNER_SECONDS = 2.4;

// (id, plain wording, wording once a tool resolves, chord, and who runs it:
// `tool` for a core action, `own` for one of the launcher's own verbs).
// `named` is what teaches the chord: "Edit" becomes "Edit in Zed".
//
// `fallbackTool` stands in for a declared tool where the platform always has an
// answer. Read lazily: the catalog is built at import time, before the platform
// has been asked what it is.
const CATALOG = [
    { id: `${PREFIX}open`, plain: 'Open', chord: '⏎', own: 'open' },
    { id: `${PREFIX}edit`, plain: 'Edit', named: 'Edit in', chord: 'Ctrl+E', tool: EDIT },
    {
        id: `${PREFIX}terminal`,
        plain: 'Open terminal here',
        named: 'Open in',
        chord: 'Ctrl+T',
        tool: TERMINAL,
    },
    {
        id: `${PREFIX}reveal`,
        plain: 'Reveal',
        named: 'Reveal in',
        chord: 'Ctrl+F',
        tool: REVEAL,
        fallbackTool: systemFileManager,
    },
    { id: `${PREFIX}copypath`, plain: 'Copy path', chord: 'Ctrl+C', own: 'copyPath' },
];

// Open and Copy path are the launcher's own verbs, not core's; keyboard.js owns
// them and registers them here so the menu and the chords run one thing.
let handlers = {};

export function setHandlers(own) {
    handlers = own;
}

/**
 * Whether `action` means anything for a row of this kind.
 *
 * Editing and opening a terminal are about a place you work in. An app is a
 * thing you launch, and the folder holding it is never "here", so both are
 * absent for app rows rather than quietly acting on the wrong directory.
 * Revealing an app is genuinely useful and stays.
 */
export function applies(action, kind) {
    const fileOrFolder = kind === 'file' || kind === 'folder';
    return action === REVEAL ? fileOrFolder || kind === 'app' : fileOrFolder;
}

/** The selected row, or null when it is not something actions apply to. */
function actionableSelection() {
    const selected = results.getSelected();
    if (!selected?.path || isSyntheticResultId(selected.id)) return null;
    if (selected.kind === 'clipboard' || selected.kind === 'process') return null;
    return selected;
}

/**
 * What the Ctrl+K menu lists for the selected row. One IPC hop for every entry
 * that needs a tool name: resolving is string work in core over the cached
 * config, so asking about them together costs one round trip in total.
 */
export async function descriptorsFor() {
    const selected = actionableSelection();
    if (!selected) return [];

    const offered = CATALOG.filter((entry) => !entry.tool || applies(entry.tool, selected.kind));
    const asked = offered.filter((entry) => entry.tool).map((entry) => entry.tool);
    const resolved = asked.length
        ? await toolActions(asked, selected.path, selected.kind === 'folder')
        : [];
    const tools = new Map(asked.map((action, index) => [action, resolved[index]?.tool]));

    return offered.map((entry) => ({
        id: entry.id,
        title: label(entry, entry.tool && tools.get(entry.tool)),
        // The chord that already performs this, set beside the label rather
        // than trailing it, so the keys line up down one edge.
        chord: entry.chord,
    }));
}

function label(entry, tool) {
    const named = tool || entry.fallbackTool?.();
    return entry.named && named ? `${entry.named} ${named}` : entry.plain;
}

/** Run one entry, by menu click or by its chord. */
export function activate(id) {
    const entry = CATALOG.find((candidate) => candidate.id === id);
    if (!entry) return;
    if (entry.tool) {
        run(entry.tool);
        return;
    }
    handlers[entry.own]?.();
}

/**
 * Perform one tool action on the selected row. The backend hides the launcher
 * before it spawns and brings it back only when nothing started, so a failure
 * has a window to report itself in.
 */
export async function run(action) {
    const selected = actionableSelection();
    if (!selected || !applies(action, selected.kind)) return;

    let outcome = null;
    try {
        outcome = await performToolAction(action, selected.path, selected.kind === 'folder');
    } catch (err) {
        console.error('tool action failed', err);
        return;
    }

    // An action that could not run explains itself here rather than being
    // greyed out with a tool name nobody could find.
    if (outcome?.reason) banner.show(outcome.reason, 'info', BANNER_SECONDS);
}
