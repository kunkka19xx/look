// Rows a user-declared block produced: which block a row came from, what it
// asked to be drawn as, and where the row can go next (specs/user-sources.md).
//
// Mirrors macOS SourceBlockCatalog / SourceBlockAction / SourceBlockIcons. What
// a block declares and what performing it does live in core; this is the cache
// that lets a row render synchronously and the dispatch that keeps a declared
// target from colliding with a compiled one.

import {
    sourceBlock,
    sourceBlocks,
    performBlock,
    getHomeDir,
    openPath,
    recordUsage,
    hideWindow,
    reloadConfig,
} from '../ipc.js';
import { zap } from '../icons.js';
import * as levels from '../levels.js';
import * as banner from './banner.js';
import * as actionmenu from './actionmenu.js';

// `look_indexing::CandidateIdKind::PREFIX_SOURCE`. A drilled row keeps it, so
// this tells indexed and drilled source rows alike from every other row.
const ID_PREFIX = 'src:';
// Namespaces a declared target inside the action menu, so it can never collide
// with a row verb or a compiled Quick Action id.
const ACTION_PREFIX = 'srcblock:';
// `look_indexing::UsageAction::EXECUTE`. Recorded like an open, so a routine
// run every morning ranks like one.
const USAGE_ACTION = 'execute';
const BANNER_SECONDS = 4.0;

// Declared icons and names, keyed by block id. Null until `prefill` lands: rows
// render synchronously and there are many of them, so a miss falls back to the
// generic glyph rather than blocking on a disk read.
let catalog = null;
// What is typed right now, which is what `{query}` expands to. Set once by
// app.js rather than threaded through every caller: inside a level it is the
// text typed AT that level, and there is only ever one input.
let queryProvider = () => '';
// The block behind a row, keyed by row rather than by block: a target's
// `confirm` is expanded against the row ("Delete main?").
const detailByRow = new Map();
const detailInFlight = new Map();

/** A row a user-declared block produced. One definition: a namespace check
 *  spelled out in three files is how one of them drifts. */
export function isSourceRow(id) {
    return typeof id === 'string' && id.startsWith(ID_PREFIX);
}

/** `src:<block>:<row>` and the drilled `src:<block>:|…|<row>` alike -> `<block>`.
 *  The one place the shell reads a candidate id, and only ever its namespace:
 *  everything past the block belongs to core. */
export function blockIdOf(candidateId) {
    if (!isSourceRow(candidateId)) return null;
    const rest = candidateId.slice(ID_PREFIX.length);
    const separator = rest.indexOf(':');
    return separator > 0 ? rest.slice(0, separator) : null;
}

export function actionIdFor(blockId) {
    return ACTION_PREFIX + blockId;
}

export function blockIdFromActionId(actionId) {
    return actionId?.startsWith(ACTION_PREFIX) ? actionId.slice(ACTION_PREFIX.length) : null;
}

export function setQueryProvider(fn) {
    queryProvider = fn;
}

/** Reads the block catalog once per launcher open, so the caches are warm
 *  before anything renders. */
export async function prefill() {
    try {
        const [blocks, dir] = await Promise.all([sourceBlocks(), getHomeDir()]);
        catalog = new Map((blocks || []).map((block) => [block.id, block]));
        // A declared icon may be written `~/…`, as readily by a user as by a
        // script, and only the backend knows what that is.
        home = dir || null;
    } catch (err) {
        console.warn('sourceblocks: could not read the catalog', err);
    }
}

/** A reload may have changed what blocks exist and what they declare. */
export function invalidate() {
    detailByRow.clear();
    detailInFlight.clear();
    // The catalog is replaced when the new one lands rather than dropped now:
    // rows render synchronously, and clearing it first paints every row with a
    // generic icon for as long as the read takes.
    return prefill();
}

/**
 * The one refresh gesture (Ctrl+Shift+;), from this half's side: the backend
 * reloads the config, re-runs every top-level `run` block and reindexes, and
 * the block a script broke is named rather than quietly producing no rows.
 *
 * A failed block keeps the rows it had, so the list the user is looking at is
 * stale rather than empty.
 */
export async function reload() {
    const outcome = await reloadConfig();
    await invalidate();
    for (const error of outcome?.errors || []) {
        banner.show(error, 'error', BANNER_SECONDS);
    }
    return outcome;
}

/** The block's name, for the row's kind label. Null before the catalog lands. */
export function blockName(candidateId) {
    const id = blockIdOf(candidateId);
    return id ? catalog?.get(id)?.name || null : null;
}

/** What the row declared, else its block: the row's own wins whatever its form. */
function declaredIcon(result) {
    return result?.icon?.trim() || blockIconOf(result?.id)?.trim() || null;
}

function blockIconOf(candidateId) {
    const id = blockIdOf(candidateId);
    return id ? catalog?.get(id)?.icon : null;
}

/**
 * A declared icon that draws as text, as HTML. An SF Symbol name has no Lucide
 * equivalent, so it falls through to the generic glyph rather than a guess.
 */
export function declaredIconHtml(result) {
    const declared = declaredIcon(result);
    if (!declared || isImagePath(declared) || isSymbolName(declared)) return null;
    return `<span class="result-icon-glyph">${escapeText(declared)}</span>`;
}

/** The image file a row is drawn as, for the icon pipeline to read. */
export function declaredIconPath(result) {
    const declared = declaredIcon(result);
    return declared && isImagePath(declared) ? expandHome(declared) : null;
}

/** The bolt, for a block row with nothing on disk: Enter performs steps. */
export const actionIconHtml = zap;

/** The block already read for this row, or null. Synchronous, for whoever
 *  already has it: the menu and the panel await `loadDetail` instead. */
export function detailFor(result) {
    return (result && detailByRow.get(cacheKey(result))) || null;
}

/** The `then` targets already read for this row. */
export function targetsFor(result) {
    return detailFor(result)?.then || [];
}

/**
 * The block behind one row: its name, the steps Enter will run, the file that
 * declared it, and where the row can go next.
 *
 * Awaited rather than answered empty while it loads: an empty first answer
 * would show the row's verbs in the menu, and the targets that beat them a
 * press later. Two callers can share the read, so an in-flight one is handed
 * the same promise.
 */
export async function loadDetail(result) {
    if (!isSourceRow(result?.id)) return null;
    const key = cacheKey(result);
    if (detailByRow.has(key)) return detailByRow.get(key);
    if (!detailInFlight.has(key)) {
        const reading = sourceBlock(rowPayload(result))
            .then((block) => {
                detailByRow.set(key, block || null);
                return block || null;
            })
            .catch((err) => {
                console.warn('sourceblocks: could not read a block', err);
                return null;
            })
            .finally(() => detailInFlight.delete(key));
        detailInFlight.set(key, reading);
    }
    return detailInFlight.get(key);
}

/** A target's `confirm` is expanded against the row, so two rows sharing an id
 *  but differing in title or path must not share an entry. */
function cacheKey(result) {
    return `${result.id}${result.title}${result.path || ''}`;
}

/** The row payload every source command takes. `ancestors` comes from the level
 *  stack, so a drilled row's `{parent.*}` reaches the rows above it. */
export function rowPayload(result) {
    return {
        candidateId: result.id,
        rowTitle: result.title,
        rowPath: result.path || '',
        query: queryProvider(),
        ancestors: levels.ancestorsJson(),
    };
}

/**
 * Enter on a block row: what the block's `open` says, or its steps when it is a
 * bundle. Core decides whether the row's own path is what opens, so the answer
 * does not come from the row's kind.
 */
export async function performRow(result) {
    const blockId = blockIdOf(result.id);
    if (!blockId) return { errors: ["couldn't tell which block this row belongs to"] };
    try {
        return await performBlock(blockId, rowPayload(result), false);
    } catch (err) {
        console.error('sourceblocks: perform failed', err);
        return { errors: ['the backend did not answer'] };
    }
}

/**
 * Enter on a block row, start to finish: perform it, open the row's path when
 * core says the row IS the thing, and record the intent either way.
 *
 * The outcome is awaited before the launcher goes away, so a step that could
 * not be started has a window to say so in. Steps are detached, so this only
 * waits for the spawn; a step's own exit code is its business.
 */
export async function activateRow(result) {
    // Resolved before anything else: an unparseable id would otherwise rank the
    // row up, close the window, and do nothing, with no way to tell it failed.
    if (!blockIdOf(result.id)) {
        banner.show("Couldn't tell which block this row belongs to", 'error', BANNER_SECONDS);
        return;
    }

    // A block that declares `confirm` asks before Enter runs it, in the menu
    // and not a modal, exactly as a `then` target does: it is the same row and
    // the same risk, reached by a different key (§2.5).
    const declared = await loadDetail(result);
    if (declared?.confirm && !(await actionmenu.askConfirm(declared.confirm))) return;

    const outcome = await performRow(result);
    // On intent, like an open: core skips a drilled row, which is transient and
    // has nothing in the candidates table to key on.
    recordUsage(result.id, USAGE_ACTION).catch(() => {});

    if (outcome?.opens_path) {
        // `open_path` hides the launcher itself, the way it does for any row.
        openPath(result.path, result.kind, result.id);
        return;
    }

    const failure = outcome?.errors?.[0];
    if (failure) {
        banner.show(`${result.title}: ${failure}`, 'error', BANNER_SECONDS);
        return;
    }
    hideWindow();
}

/**
 * Running a `then` target against the selected row, which is what its
 * placeholders expand to. A target that produces rows is a level to descend
 * into rather than something to run, and core is what says which.
 */
export async function performTarget({ blockId, title, result, selectionId }) {
    // Claimed before the block runs: the user can hide the launcher or start
    // another target while it does.
    const token = levels.beginRequest();
    const ancestors = levels.ancestorsJson();
    const parent = {
        candidateId: result.id,
        title: result.title,
        path: result.path || '',
        openedFromQuery: queryProvider(),
        openedFromSelection: selectionId,
    };

    let outcome;
    try {
        outcome = await performBlock(blockId, rowPayload(result), true);
    } catch (err) {
        console.error('sourceblocks: target failed', err);
        return { error: 'the backend did not answer' };
    }
    if (!levels.holds(token)) return { stale: true };

    const failure = outcome?.errors?.[0];
    if (failure) return { error: failure };
    if (!outcome?.produces_rows) {
        // It ran. The steps usually bring something up that wants the focus.
        hideWindow();
        return { performed: true };
    }

    // Not a failure and nothing performed: the target lists, so it is a level.
    const level = await levels.descend({ blockId, title, parent, ancestors, token });
    return { level };
}

const IMAGE_EXTENSIONS = ['.png', '.jpg', '.jpeg', '.gif', '.svg', '.webp', '.bmp', '.ico'];

function isImagePath(declared) {
    const lowered = declared.toLowerCase();
    return (
        (declared.startsWith('/') || declared.startsWith('~/') || declared.startsWith('.')) &&
        IMAGE_EXTENSIONS.some((extension) => lowered.endsWith(extension))
    );
}

// "bolt.fill", "folder.badge.gear": a macOS symbol name, which has no Lucide
// counterpart. Emoji have no ASCII letters, so this never catches one.
function isSymbolName(declared) {
    return /^[a-z0-9.]+$/i.test(declared);
}

let home = null;

/** `/home/you/.look/sources/x.toml` -> `~/.look/sources/x.toml`, the way macOS
 *  abbreviates the declaring file in the panel. */
export function tildePath(path) {
    return home && path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}

function expandHome(path) {
    return path.startsWith('~/') && home ? `${home}${path.slice(1)}` : path;
}

function escapeText(value) {
    return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
}
