// The verbs a row with a path offers, and the tools that carry them out.
//
// Mirrors macOS LauncherView+RowActions and LauncherView+Tools: one declarative
// table plus one eligibility rule, read by both the Ctrl+K menu and the chords,
// so the two can never drift. Composition lives in core (look-tools) and the
// wording of an action that cannot run comes back with it - nothing here writes
// a message about a missing tool.

import { toolAction, performToolAction } from '../ipc.js';
import * as results from './results.js';
import * as banner from './banner.js';
import { isSyntheticResultId } from '../catalog.js';
import { systemFileManager } from '../platform.js';

// Core's action ids (look_tools::Action::id).
export const EDIT = 'edit';
export const TERMINAL = 'terminal';
export const REVEAL = 'reveal';

const PREFIX = 'rowaction:';

export const ID = {
    open: `${PREFIX}open`,
    edit: `${PREFIX}edit`,
    terminal: `${PREFIX}terminal`,
    reveal: `${PREFIX}reveal`,
    copyPath: `${PREFIX}copypath`,
};

const BANNER_SECONDS = 2.4;

// (id, plain wording, wording once a tool resolves, chord, tool action). `named`
// is what teaches the chord: "Edit" becomes "Edit in Zed".
//
// `fallbackTool` stands in for a declared tool where the platform always has an
// answer. Read lazily: the catalog is built at import time, before the platform
// has been asked what it is.
const CATALOG = [
    { id: ID.open, plain: 'Open', chord: '⏎' },
    { id: ID.edit, plain: 'Edit', named: 'Edit in', chord: 'Ctrl+E', tool: EDIT },
    {
        id: ID.terminal,
        plain: 'Open terminal here',
        named: 'Open in',
        chord: 'Ctrl+T',
        tool: TERMINAL,
    },
    {
        id: ID.reveal,
        plain: 'Reveal',
        named: 'Reveal in',
        chord: 'Ctrl+F',
        tool: REVEAL,
        fallbackTool: systemFileManager,
    },
    { id: ID.copyPath, plain: 'Copy path', chord: 'Ctrl+C' },
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

/** The selected row as a target, or null when no action applies to it. */
function target(action) {
    const selected = results.getSelected();
    if (!selected?.path || isSyntheticResultId(selected.id)) return null;
    if (action && !applies(action, selected.kind)) return null;
    return { path: selected.path, isDir: selected.kind === 'folder' };
}

/**
 * What the Ctrl+K menu lists for `result`. Resolving is string work in core over
 * the cached config, so the three lookups cost one round trip between them.
 */
export async function descriptorsFor(result) {
    if (!result?.path || result.kind === 'clipboard' || result.kind === 'process') return [];
    if (isSyntheticResultId(result.id)) return [];

    const offered = CATALOG.filter((entry) => !entry.tool || applies(entry.tool, result.kind));
    const resolved = await Promise.all(
        offered.map((entry) =>
            entry.tool
                ? toolAction(entry.tool, result.path, result.kind === 'folder')
                : Promise.resolve(null),
        ),
    );

    return offered.map((entry, index) => ({
        id: entry.id,
        title: label(entry, resolved[index]?.tool),
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
export function activate(actionId) {
    switch (actionId) {
        case ID.open:
            handlers.open?.();
            break;
        case ID.edit:
            run(EDIT);
            break;
        case ID.terminal:
            run(TERMINAL);
            break;
        case ID.reveal:
            run(REVEAL);
            break;
        case ID.copyPath:
            handlers.copyPath?.();
            break;
    }
}

/**
 * Perform one tool action on the selected row. The backend hides the launcher
 * before it spawns and brings it back only when nothing started, so a failure
 * has a window to report itself in.
 */
export async function run(action) {
    const row = target(action);
    if (!row) return;

    let outcome = null;
    try {
        outcome = await performToolAction(action, row.path, row.isDir);
    } catch (err) {
        console.error('tool action failed', err);
        return;
    }

    // An action that could not run explains itself here rather than being
    // greyed out with a tool name nobody could find.
    if (outcome?.reason) banner.show(outcome.reason, 'info', BANNER_SECONDS);
}
