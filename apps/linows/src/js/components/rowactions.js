// The verbs a row offers, and the tools that carry them out.
//
// Mirrors macOS LauncherView+RowActions and LauncherView+Tools: one declarative
// table plus one eligibility rule, read by both the Ctrl+K menu and the chords,
// so the two can never drift. Composition lives in core (look-tools) and the
// wording of an action that cannot run comes back with it - nothing here writes
// a message about a missing tool.
//
// A row a block produced also carries what the block declared: its own
// `edit` / `terminal` / `reveal` beat the user's global tool, and its `then`
// targets join this list.

import { toolActions, performToolAction } from '../ipc.js';
import * as results from './results.js';
import * as banner from './banner.js';
import * as sourceblocks from './sourceblocks.js';
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
 *
 * A block's row qualifies too: one that names a `path` (`format = "json"`) is a
 * real filesystem object. The callers all require a path, so a row that names
 * none never reaches here.
 */
export function applies(action, kind) {
    const place = kind === 'file' || kind === 'folder' || kind === 'action';
    return action === REVEAL ? place || kind === 'app' : place;
}

/** The selected row, or null when it is not something actions apply to. */
function actionableSelection() {
    const selected = results.getSelected();
    if (!selected || isSyntheticResultId(selected.id)) return null;
    if (selected.kind === 'clipboard' || selected.kind === 'process') return null;
    // A block row with nothing on disk still has targets and a declaring file,
    // so it stays actionable; the path-taking verbs check for themselves.
    if (!selected.path && !sourceblocks.isSourceRow(selected.id)) return null;
    return selected;
}

/**
 * What the Ctrl+K menu lists for the selected row. One IPC hop for every entry
 * that needs a tool name: resolving is string work in core over the cached
 * config, so asking about them together costs one round trip in total.
 *
 * A block that declared `then` targets shows those instead of the verb list:
 * declared beats default in the menu exactly as it does when running, and the
 * chords still carry every verb regardless.
 */
export async function descriptorsFor() {
    const selected = actionableSelection();
    if (!selected) return [];

    const block = await sourceblocks.loadDetail(selected);
    const targets = (block?.then || []).map((target) => ({
        id: sourceblocks.actionIdFor(target.id),
        // The ellipsis says this one lists rather than runs: it opens a level.
        title: target.performs ? target.name : `${target.name}…`,
        // Already expanded against the row, so the question names what will
        // actually happen. The menu asks it in place of the action list.
        confirm: target.confirm,
    }));
    if (targets.length > 0) return targets;

    if (!selected.path) return [];
    const offered = CATALOG.filter((entry) => !entry.tool || applies(entry.tool, selected.kind));
    const asked = offered.filter((entry) => entry.tool).map((entry) => entry.tool);
    const resolved = asked.length
        ? await toolActions(asked, rowFor(selected), isDir(selected))
        : [];
    const tools = new Map(asked.map((action, index) => [action, resolved[index]]));

    return offered.map((entry) => ({
        id: entry.id,
        title: label(entry, entry.tool && tools.get(entry.tool)),
        // The chord that already performs this, set beside the label rather
        // than trailing it, so the keys line up down one edge.
        chord: entry.chord,
    }));
}

// "Edit in Zed", or "Edit in Projects" when the row's own block took the chord:
// core says which, so a block name never gets read as a tool name.
function label(entry, resolved) {
    const named = resolved?.tool || entry.fallbackTool?.();
    return entry.named && named ? `${entry.named} ${named}` : entry.plain;
}

/** Run one entry, by menu click or by its chord. */
export function activate(id) {
    const blockId = sourceblocks.blockIdFromActionId(id);
    if (blockId) {
        runTarget(blockId, id);
        return;
    }

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
    if (!selected?.path || !applies(action, selected.kind)) return;

    let outcome = null;
    try {
        outcome = await performToolAction(action, rowFor(selected), isDir(selected));
    } catch (err) {
        console.error('tool action failed', err);
        return;
    }

    // An action that could not run explains itself here rather than being
    // greyed out with a tool name nobody could find.
    if (outcome?.reason) banner.show(outcome.reason, 'info', BANNER_SECONDS);
}

/** A `then` target: performed, or descended into when it produces rows. */
async function runTarget(blockId, actionId) {
    const selected = actionableSelection();
    if (!selected) return;
    const target = sourceblocks
        .targetsFor(selected)
        .find((candidate) => sourceblocks.actionIdFor(candidate.id) === actionId);
    const title = target?.name || blockId;

    const outcome = await sourceblocks.performTarget({
        blockId,
        title,
        result: selected,
        selectionId: selected.id,
    });
    if (outcome?.error) {
        banner.show(`${title}: ${outcome.error}`, 'error', BANNER_SECONDS);
        return;
    }
    const level = outcome?.level;
    if (level?.error) {
        // An empty level and a broken command look identical once you are
        // inside one, so neither is entered.
        banner.show(`${title}: ${level.error}`, 'error', BANNER_SECONDS);
    } else if (level?.truncated) {
        banner.show(`${title}: showing the first ${level.count}`, 'info', BANNER_SECONDS);
    }
}

/** The row as core needs it: its id so a block can override the chord, its
 *  title and ancestors so that override expands like any other command. */
function rowFor(selected) {
    return sourceblocks.rowPayload(selected);
}

/** A file row and a folder row say which they are; a block's row does not, so
 *  only the filesystem knows and the backend is what asks it. */
function isDir(selected) {
    return selected.kind === 'action' ? null : selected.kind === 'folder';
}
