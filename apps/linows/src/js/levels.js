// The levels a user has descended into from a block row (specs/user-sources.md
// §2.10).
//
// Port of macOS SourceLevelStack: one value holding the navigation state rather
// than another handful of booleans beside the search modes. A level owns the
// result list while it is up, because its rows are produced live and are not in
// the index.

import { sourceRows } from './ipc.js';

let frames = [];
// Bumped by every request and every change, so a target still running after the
// launcher is hidden, or after another starts, answers stale.
let epoch = 0;
// Told when a level is pushed: the launcher has to hand it the result list,
// clear the query and say where it is.
let onEnter = null;

export function setOnEnter(fn) {
    onEnter = fn;
}

export function isActive() {
    return frames.length > 0;
}

export function current() {
    return frames.length > 0 ? frames[frames.length - 1] : null;
}

/** "Projects > animate > Scripts": the row you came from does not say which of
 *  its targets you picked, so the block that produced this level is named. */
export function breadcrumb() {
    const level = current();
    if (!level) return [];
    return [...frames.map((frame) => frame.parentTitle), level.blockName];
}

/** Ancestors of the rows IN this level, nearest first, for `{parent.*}`. */
export function ancestorsJson() {
    return JSON.stringify(
        [...frames].reverse().map((frame) => ({
            id: frame.parentRowId,
            title: frame.parentTitle,
            path: frame.parentPath,
        })),
    );
}

/** The epoch a request must still hold when it answers. */
export function beginRequest() {
    epoch += 1;
    return epoch;
}

export function holds(token) {
    return token === epoch;
}

export function pop() {
    epoch += 1;
    return frames.pop() || null;
}

export function clear() {
    // Bumped even with no frames: a first-level request may be in flight.
    epoch += 1;
    frames = [];
}

/**
 * The rows of the current level, filtered by what is typed at it.
 *
 * Not the engine's scorer: these rows are a list the user is looking at, one
 * block's output against one parent, and narrowing it must not reorder what the
 * block's author wrote (`--sort=-committerdate` beats a frecency guess).
 */
export function rows(query) {
    const level = current();
    if (!level) return [];
    const needle = query.trim().toLowerCase();

    return level.rows
        .filter((row) => matches(needle, row))
        .map((row, position) => ({
            id: row.candidateId,
            // Enter runs the block's verbs whatever the row carries; a path
            // only says where the chords act.
            kind: 'action',
            title: row.title,
            subtitle: row.subtitle,
            path: row.path || '',
            // Descending, so the producer's order IS the score wherever one is
            // read (macOS LauncherView+Levels does the same).
            score: level.rows.length - position,
            icon: row.icon,
        }));
}

function matches(needle, row) {
    if (!needle) return true;
    return (
        row.title.toLowerCase().includes(needle) ||
        row.id.toLowerCase().includes(needle) ||
        (row.subtitle || '').toLowerCase().includes(needle)
    );
}

/**
 * Opens `blockId` as a level below `parent`, which is passed in rather than
 * read from the selection: the target that led here ran detached, and the user
 * may have moved on.
 *
 * Returns `{ok, error, truncated, count}`. A failure does not descend, and
 * neither does an empty result: once you are inside one, they look identical.
 */
export async function descend({ blockId, title, parent, ancestors, token }) {
    let level;
    try {
        level = await sourceRows(blockId, {
            candidateId: parent.candidateId,
            rowTitle: parent.title,
            rowPath: parent.path,
            query: parent.openedFromQuery,
            ancestors,
        });
    } catch (err) {
        console.error('levels: could not read', blockId, err);
        return { ok: false, error: 'the backend did not answer' };
    }

    if (!holds(token)) return { ok: false, stale: true };
    if (!level) return { ok: false, error: 'the backend did not answer' };
    if (level.error) return { ok: false, error: level.error };

    frames.push({
        blockName: title,
        // The row's OWN id, decoded by core, which is what `{parent.id}`
        // expands to.
        parentRowId: level.parentRowId,
        parentTitle: parent.title,
        parentPath: parent.path,
        rows: level.rows,
        restoredQuery: parent.openedFromQuery,
        restoredSelectionId: parent.openedFromSelection,
    });
    epoch += 1;
    onEnter?.();

    return { ok: true, truncated: level.truncated, count: level.rows.length };
}
